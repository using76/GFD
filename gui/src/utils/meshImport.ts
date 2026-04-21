/**
 * Unified mesh file parser for STL and 3DM formats.
 * Replaces 4 duplicate parsers across Ribbon, MenuBar, PrimitiveToolbar, Viewport3D.
 *
 * Key improvement: preserves per-face normals from STL for flat shading (sharp edges).
 */

export interface ParsedMesh {
  vertices: Float32Array;   // xyz per vertex, 9 floats per triangle
  normals: Float32Array;    // per-face normal copied to each vertex (flat shading)
  faceCount: number;
}

// ============================================================
// STL Parsing
// ============================================================

/** Parse binary STL, preserving per-face normals */
function parseBinaryStl(buf: ArrayBuffer): ParsedMesh {
  const dv = new DataView(buf);
  const faceCount = dv.getUint32(80, true);
  if (faceCount === 0 || 84 + faceCount * 50 > buf.byteLength) {
    return { vertices: new Float32Array(0), normals: new Float32Array(0), faceCount: 0 };
  }
  const vertices = new Float32Array(faceCount * 9);
  const normals = new Float32Array(faceCount * 9);
  let offset = 84;
  for (let i = 0; i < faceCount; i++) {
    // Read face normal
    const nx = dv.getFloat32(offset, true);
    const ny = dv.getFloat32(offset + 4, true);
    const nz = dv.getFloat32(offset + 8, true);
    offset += 12;
    // Read 3 vertices, copy normal to each
    for (let v = 0; v < 3; v++) {
      const idx = i * 9 + v * 3;
      vertices[idx]     = dv.getFloat32(offset, true);
      vertices[idx + 1] = dv.getFloat32(offset + 4, true);
      vertices[idx + 2] = dv.getFloat32(offset + 8, true);
      normals[idx]     = nx;
      normals[idx + 1] = ny;
      normals[idx + 2] = nz;
      offset += 12;
    }
    offset += 2; // attribute byte count
  }
  return { vertices, normals, faceCount };
}

/** Parse ASCII STL, computing per-face normals */
function parseAsciiStl(text: string): ParsedMesh {
  const facetNormalRegex = /facet\s+normal\s+([-\d.eE+]+)\s+([-\d.eE+]+)\s+([-\d.eE+]+)/gi;
  const vertexRegex = /vertex\s+([-\d.eE+]+)\s+([-\d.eE+]+)\s+([-\d.eE+]+)/gi;

  const faceNormals: number[] = [];
  let m: RegExpExecArray | null;
  while ((m = facetNormalRegex.exec(text)) !== null) {
    faceNormals.push(parseFloat(m[1]), parseFloat(m[2]), parseFloat(m[3]));
  }

  const coords: number[] = [];
  while ((m = vertexRegex.exec(text)) !== null) {
    coords.push(parseFloat(m[1]), parseFloat(m[2]), parseFloat(m[3]));
  }

  const faceCount = Math.floor(coords.length / 9);
  const vertices = new Float32Array(coords);
  const normals = new Float32Array(faceCount * 9);

  for (let i = 0; i < faceCount; i++) {
    let nx: number, ny: number, nz: number;
    if (i < faceNormals.length / 3) {
      nx = faceNormals[i * 3]; ny = faceNormals[i * 3 + 1]; nz = faceNormals[i * 3 + 2];
    } else {
      // Compute from cross product if normal not in file
      const i0 = i * 9;
      const ax = coords[i0+3]-coords[i0], ay = coords[i0+4]-coords[i0+1], az = coords[i0+5]-coords[i0+2];
      const bx = coords[i0+6]-coords[i0], by = coords[i0+7]-coords[i0+1], bz = coords[i0+8]-coords[i0+2];
      nx = ay*bz-az*by; ny = az*bx-ax*bz; nz = ax*by-ay*bx;
      const len = Math.sqrt(nx*nx+ny*ny+nz*nz) || 1;
      nx /= len; ny /= len; nz /= len;
    }
    for (let v = 0; v < 3; v++) {
      normals[i*9+v*3] = nx; normals[i*9+v*3+1] = ny; normals[i*9+v*3+2] = nz;
    }
  }

  return { vertices, normals, faceCount };
}

/** Auto-detect and parse STL (binary or ASCII) */
export function parseStl(buf: ArrayBuffer): ParsedMesh {
  const headerBytes = new Uint8Array(buf, 0, Math.min(6, buf.byteLength));
  const headerStr = String.fromCharCode(...headerBytes);

  if (headerStr.startsWith('solid') && buf.byteLength > 84) {
    const text = new TextDecoder().decode(buf);
    if (text.includes('vertex')) {
      const result = parseAsciiStl(text);
      if (result.faceCount > 0) return result;
    }
  }
  return parseBinaryStl(buf);
}

// ============================================================
// 3DM Parsing (rhino3dm WASM)
// ============================================================

let rhinoModule: typeof import('rhino3dm') | null = null;

/** Parse Rhino .3dm file, returning triangulated mesh */
export async function parse3dm(buf: ArrayBuffer): Promise<ParsedMesh> {
  // Dynamic import to avoid loading WASM unless needed
  if (!rhinoModule) {
    const mod = await import('rhino3dm');
    rhinoModule = mod;
  }
  const rhino = await (rhinoModule as { default: () => Promise<unknown> }).default();
  const rh = rhino as {
    File3dm: { fromByteArray(data: Uint8Array): {
      objects(): { count: number; get(i: number): { geometry(): unknown; attributes(): { name: string } } };
    } | null };
    ObjectType: { Brep: number; Extrusion: number; Mesh: number };
  };

  const file3dm = rh.File3dm.fromByteArray(new Uint8Array(buf));
  if (!file3dm) throw new Error('Failed to parse .3dm file');

  const objects = file3dm.objects();
  const allVerts: number[] = [];
  const allNormals: number[] = [];

  for (let oi = 0; oi < objects.count; oi++) {
    const obj = objects.get(oi);
    const geo = obj.geometry() as {
      objectType: number;
      faces?(): { count: number; get(i: number): { domain(dir: number): [number, number] | null; pointAt(u: number, v: number): number[] | null } };
      getMesh?(density: number): { vertices(): { count: number; get(i: number): number[] }; faces(): { count: number; get(i: number): number[] } } | null;
    };

    // Try to get mesh from Brep via tessellation
    if (geo.faces && typeof geo.faces === 'function') {
      const faces = geo.faces();
      for (let fi = 0; fi < faces.count; fi++) {
        try {
          const face = faces.get(fi);
          const uDom = face.domain(0);
          const vDom = face.domain(1);
          if (!uDom || !vDom) continue;
          const u0 = uDom[0], u1 = uDom[1], v0 = vDom[0], v1 = vDom[1];
          if (u1 - u0 <= 0 || v1 - v0 <= 0) continue;

          // Estimate physical size for adaptive subdivision
          const p00 = face.pointAt(u0, v0);
          const p10 = face.pointAt(u1, v0);
          const p01 = face.pointAt(u0, v1);
          if (!p00 || !p10 || !p01) continue;

          const uSpan = Math.sqrt((p10[0]-p00[0])**2 + (p10[1]-p00[1])**2 + (p10[2]-p00[2])**2);
          const vSpan = Math.sqrt((p01[0]-p00[0])**2 + (p01[1]-p00[1])**2 + (p01[2]-p00[2])**2);
          const targetEdge = 30;
          const nu = Math.min(20, Math.max(3, Math.ceil(uSpan / targetEdge)));
          const nv = Math.min(20, Math.max(3, Math.ceil(vSpan / targetEdge)));
          const du = (u1 - u0) / nu, dv = (v1 - v0) / nv;

          // Sample grid
          const grid: (number[] | null)[][] = [];
          for (let vi = 0; vi <= nv; vi++) {
            const row: (number[] | null)[] = [];
            for (let ui = 0; ui <= nu; ui++) {
              const pt = face.pointAt(u0 + ui * du, v0 + vi * dv);
              row.push(pt && pt.length >= 3 ? [pt[0], pt[1], pt[2]] : null);
            }
            grid.push(row);
          }

          // Triangulate grid
          for (let vi = 0; vi < nv; vi++) {
            for (let ui = 0; ui < nu; ui++) {
              const pa = grid[vi][ui], pb = grid[vi][ui+1];
              const pc = grid[vi+1][ui], pd = grid[vi+1][ui+1];
              if (pa && pb && pc) {
                const ax=pb[0]-pa[0], ay=pb[1]-pa[1], az=pb[2]-pa[2];
                const bx=pc[0]-pa[0], by=pc[1]-pa[1], bz=pc[2]-pa[2];
                let nx=ay*bz-az*by, ny=az*bx-ax*bz, nz=ax*by-ay*bx;
                const len = Math.sqrt(nx*nx+ny*ny+nz*nz) || 1;
                nx/=len; ny/=len; nz/=len;
                allVerts.push(...pa, ...pb, ...pc);
                allNormals.push(nx,ny,nz, nx,ny,nz, nx,ny,nz);
              }
              if (pb && pd && pc) {
                const ax=pd[0]-pb[0], ay=pd[1]-pb[1], az=pd[2]-pb[2];
                const bx=pc[0]-pb[0], by=pc[1]-pb[1], bz=pc[2]-pb[2];
                let nx=ay*bz-az*by, ny=az*bx-ax*bz, nz=ax*by-ay*bx;
                const len = Math.sqrt(nx*nx+ny*ny+nz*nz) || 1;
                nx/=len; ny/=len; nz/=len;
                allVerts.push(...pb, ...pd, ...pc);
                allNormals.push(nx,ny,nz, nx,ny,nz, nx,ny,nz);
              }
            }
          }
        } catch { /* skip bad face */ }
      }
    }
  }

  const faceCount = allVerts.length / 9;
  return {
    vertices: new Float32Array(allVerts),
    normals: new Float32Array(allNormals),
    faceCount,
  };
}

// ============================================================
// Unified file parser
// ============================================================

/** Parse any supported CAD file, returning mesh data with normals */
export async function parseFile(file: File): Promise<ParsedMesh> {
  const buf = await file.arrayBuffer();
  const ext = file.name.split('.').pop()?.toLowerCase() ?? '';

  if (ext === '3dm') {
    return parse3dm(buf);
  }
  return parseStl(buf);
}

// ============================================================
// Geometry utilities
// ============================================================

/** Compute bounding box of vertices */
export function computeBBox(vertices: Float32Array) {
  let minX = Infinity, maxX = -Infinity;
  let minY = Infinity, maxY = -Infinity;
  let minZ = Infinity, maxZ = -Infinity;
  for (let i = 0; i < vertices.length; i += 3) {
    if (vertices[i] < minX) minX = vertices[i]; if (vertices[i] > maxX) maxX = vertices[i];
    if (vertices[i+1] < minY) minY = vertices[i+1]; if (vertices[i+1] > maxY) maxY = vertices[i+1];
    if (vertices[i+2] < minZ) minZ = vertices[i+2]; if (vertices[i+2] > maxZ) maxZ = vertices[i+2];
  }
  return { minX, maxX, minY, maxY, minZ, maxZ,
    cx: (minX+maxX)/2, cy: (minY+maxY)/2, cz: (minZ+maxZ)/2,
    span: Math.max(maxX-minX, maxY-minY, maxZ-minZ) };
}

/** Center vertices at origin */
export function centerVertices(vertices: Float32Array) {
  const bb = computeBBox(vertices);
  for (let i = 0; i < vertices.length; i += 3) {
    vertices[i] -= bb.cx; vertices[i+1] -= bb.cy; vertices[i+2] -= bb.cz;
  }
  return bb;
}

/** Scale vertices so max span = targetSize */
export function scaleVertices(vertices: Float32Array, targetSize: number) {
  const bb = computeBBox(vertices);
  if (bb.span <= 0) return 1;
  const scale = targetSize / bb.span;
  for (let i = 0; i < vertices.length; i++) vertices[i] *= scale;
  return scale;
}

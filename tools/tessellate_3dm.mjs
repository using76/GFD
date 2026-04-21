#!/usr/bin/env node
/**
 * Adaptive BREP tessellation from .3dm file
 * - Adapts subdivision per face based on domain extent and curvature
 * - Outputs binary STL
 *
 * Usage: node tools/tessellate_3dm.mjs <input.3dm> <output.stl> [--min-sub 3] [--max-sub 16] [--target-edge 30]
 */
import rhino3dmInit from '../gui/node_modules/rhino3dm/rhino3dm.js';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);

let inFile = null, outFile = null;
let minSub = 3, maxSub = 20, targetEdge = 30; // targetEdge in original units (mm)
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--min-sub') { minSub = parseInt(args[++i]); continue; }
  if (args[i] === '--max-sub') { maxSub = parseInt(args[++i]); continue; }
  if (args[i] === '--target-edge') { targetEdge = parseFloat(args[++i]); continue; }
  if (!inFile) { inFile = args[i]; continue; }
  if (!outFile) { outFile = args[i]; continue; }
}
if (!inFile || !outFile) {
  console.error('Usage: node tools/tessellate_3dm.mjs <in.3dm> <out.stl> [--target-edge 30]');
  process.exit(1);
}

async function main() {
  const rhino = await rhino3dmInit();
  const buf = fs.readFileSync(inFile);
  const file3dm = rhino.File3dm.fromByteArray(new Uint8Array(buf));
  if (!file3dm) { console.error('Cannot parse .3dm'); process.exit(1); }

  const objects = file3dm.objects();
  console.log(`Objects: ${objects.count}`);

  const allTris = []; // [{v0,v1,v2,nx,ny,nz}]

  for (let oi = 0; oi < objects.count; oi++) {
    const obj = objects.get(oi);
    const geo = obj.geometry();
    const attrs = obj.attributes();
    console.log(`\nObject ${oi}: "${attrs.name}" type=${geo.objectType === rhino.ObjectType.Brep ? 'Brep' : geo.objectType}`);

    if (geo.objectType !== rhino.ObjectType.Brep) {
      console.log('  Skipping non-Brep');
      continue;
    }

    const faces = geo.faces();
    console.log(`  ${faces.count} BREP faces`);

    let facesOk = 0, facesFail = 0;
    for (let fi = 0; fi < faces.count; fi++) {
      if (fi % 2000 === 0 && fi > 0) process.stdout.write(`  face ${fi}/${faces.count}...\r`);
      try {
        const face = faces.get(fi);
        const uDom = face.domain(0);
        const vDom = face.domain(1);
        if (!uDom || !vDom) { facesFail++; continue; }

        const u0 = uDom[0], u1 = uDom[1];
        const v0 = vDom[0], v1 = vDom[1];
        if (u1 - u0 <= 0 || v1 - v0 <= 0) { facesFail++; continue; }

        // Estimate physical size by sampling corners
        const p00 = face.pointAt(u0, v0);
        const p10 = face.pointAt(u1, v0);
        const p01 = face.pointAt(u0, v1);
        if (!p00 || !p10 || !p01) { facesFail++; continue; }

        const uSpan = Math.sqrt((p10[0]-p00[0])**2 + (p10[1]-p00[1])**2 + (p10[2]-p00[2])**2);
        const vSpan = Math.sqrt((p01[0]-p00[0])**2 + (p01[1]-p00[1])**2 + (p01[2]-p00[2])**2);

        // Adapt subdivision: more divisions for larger faces
        const nu = Math.min(maxSub, Math.max(minSub, Math.ceil(uSpan / targetEdge)));
        const nv = Math.min(maxSub, Math.max(minSub, Math.ceil(vSpan / targetEdge)));

        const du = (u1 - u0) / nu;
        const dv = (v1 - v0) / nv;

        // Sample grid
        const grid = [];
        for (let vi = 0; vi <= nv; vi++) {
          const row = [];
          for (let ui = 0; ui <= nu; ui++) {
            const pt = face.pointAt(u0 + ui * du, v0 + vi * dv);
            row.push(pt && pt.length >= 3 ? [pt[0], pt[1], pt[2]] : null);
          }
          grid.push(row);
        }

        // Triangulate
        for (let vi = 0; vi < nv; vi++) {
          for (let ui = 0; ui < nu; ui++) {
            const p00 = grid[vi][ui], p10 = grid[vi][ui+1];
            const p01 = grid[vi+1][ui], p11 = grid[vi+1][ui+1];

            if (p00 && p10 && p01) {
              const ax=p10[0]-p00[0],ay=p10[1]-p00[1],az=p10[2]-p00[2];
              const bx=p01[0]-p00[0],by=p01[1]-p00[1],bz=p01[2]-p00[2];
              const nx=ay*bz-az*by, ny=az*bx-ax*bz, nz=ax*by-ay*bx;
              const len = Math.sqrt(nx*nx+ny*ny+nz*nz);
              if (len > 1e-10) {
                allTris.push({ v: [p00,p10,p01], n: [nx/len,ny/len,nz/len] });
              }
            }
            if (p10 && p11 && p01) {
              const ax=p11[0]-p10[0],ay=p11[1]-p10[1],az=p11[2]-p10[2];
              const bx=p01[0]-p10[0],by=p01[1]-p10[1],bz=p01[2]-p10[2];
              const nx=ay*bz-az*by, ny=az*bx-ax*bz, nz=ax*by-ay*bx;
              const len = Math.sqrt(nx*nx+ny*ny+nz*nz);
              if (len > 1e-10) {
                allTris.push({ v: [p10,p11,p01], n: [nx/len,ny/len,nz/len] });
              }
            }
          }
        }
        facesOk++;
      } catch {
        facesFail++;
      }
    }
    console.log(`\n  OK: ${facesOk}, Failed: ${facesFail}`);
  }

  console.log(`\nTotal triangles: ${allTris.length}`);

  // Write binary STL
  const stlBuf = Buffer.alloc(84 + allTris.length * 50);
  stlBuf.write('GFD adaptive tessellation', 0);
  stlBuf.writeUInt32LE(allTris.length, 80);
  for (let t = 0; t < allTris.length; t++) {
    const off = 84 + t * 50;
    const tri = allTris[t];
    stlBuf.writeFloatLE(tri.n[0], off);
    stlBuf.writeFloatLE(tri.n[1], off+4);
    stlBuf.writeFloatLE(tri.n[2], off+8);
    for (let vi = 0; vi < 3; vi++) {
      stlBuf.writeFloatLE(tri.v[vi][0], off+12+vi*12);
      stlBuf.writeFloatLE(tri.v[vi][1], off+16+vi*12);
      stlBuf.writeFloatLE(tri.v[vi][2], off+20+vi*12);
    }
    stlBuf.writeUInt16LE(0, off+48);
  }
  fs.writeFileSync(outFile, stlBuf);
  console.log(`Written: ${outFile} (${(stlBuf.length/1024/1024).toFixed(1)} MB)`);
}

main().catch(err => { console.error(err); process.exit(1); });

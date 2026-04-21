#!/usr/bin/env node
/**
 * Simple vertex-clustering mesh decimation for STL files.
 * Reduces triangle count by merging vertices within grid cells.
 *
 * Usage: node tools/decimate_stl.mjs <input.stl> <output.stl> [--ratio 0.1]
 */
import fs from 'fs';

const args = process.argv.slice(2);
let inFile = null, outFile = null, ratio = 0.1;
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--ratio') { ratio = parseFloat(args[++i]); continue; }
  if (!inFile) { inFile = args[i]; continue; }
  if (!outFile) { outFile = args[i]; continue; }
}
if (!inFile || !outFile) { console.error('Usage: decimate_stl.mjs <in> <out> [--ratio 0.1]'); process.exit(1); }

// Read
const buf = fs.readFileSync(inFile);
const fc = buf.readUInt32LE(80);
const v = new Float32Array(fc * 9);
for (let i = 0; i < fc; i++) {
  const off = 84 + i * 50;
  for (let j = 0; j < 9; j++) v[i*9+j] = buf.readFloatLE(off + 12 + (j%3===0?0:j%3===1?4:8) + Math.floor(j/3)*12);
}
console.log(`Input: ${fc} triangles`);

// BBox
let mn = [Infinity,Infinity,Infinity], mx = [-Infinity,-Infinity,-Infinity];
for (let i = 0; i < v.length; i += 3) {
  for (let d = 0; d < 3; d++) { if (v[i+d]<mn[d]) mn[d]=v[i+d]; if (v[i+d]>mx[d]) mx[d]=v[i+d]; }
}

// Grid cell size for vertex clustering
const spans = mn.map((m,i) => mx[i]-m);
const maxSpan = Math.max(...spans);
const gridSize = maxSpan * Math.cbrt(ratio); // approx target
console.log(`Grid cell: ${gridSize.toFixed(4)}`);

// Cluster vertices
const vertMap = new Map(); // "ix,iy,iz" → averaged vertex index
const uniqueVerts = []; // [x,y,z]

function quantize(x, y, z) {
  const ix = Math.round((x - mn[0]) / gridSize);
  const iy = Math.round((y - mn[1]) / gridSize);
  const iz = Math.round((z - mn[2]) / gridSize);
  const key = `${ix},${iy},${iz}`;
  if (vertMap.has(key)) return vertMap.get(key);
  const idx = uniqueVerts.length;
  uniqueVerts.push([x, y, z]);
  vertMap.set(key, idx);
  return idx;
}

// Build deduplicated triangles
const outTris = [];
for (let i = 0; i < fc; i++) {
  const i0 = quantize(v[i*9], v[i*9+1], v[i*9+2]);
  const i1 = quantize(v[i*9+3], v[i*9+4], v[i*9+5]);
  const i2 = quantize(v[i*9+6], v[i*9+7], v[i*9+8]);
  // Skip degenerate
  if (i0 === i1 || i1 === i2 || i0 === i2) continue;
  outTris.push([i0, i1, i2]);
}

console.log(`Output: ${outTris.length} triangles (${(outTris.length/fc*100).toFixed(1)}%), ${uniqueVerts.length} vertices`);

// Write binary STL
const outBuf = Buffer.alloc(84 + outTris.length * 50);
outBuf.write('Decimated STL', 0);
outBuf.writeUInt32LE(outTris.length, 80);
for (let t = 0; t < outTris.length; t++) {
  const off = 84 + t * 50;
  const [a, b, c] = outTris[t];
  const va = uniqueVerts[a], vb = uniqueVerts[b], vc = uniqueVerts[c];
  // Normal
  const ax=vb[0]-va[0],ay=vb[1]-va[1],az=vb[2]-va[2];
  const bx=vc[0]-va[0],by=vc[1]-va[1],bz=vc[2]-va[2];
  const nx=ay*bz-az*by, ny=az*bx-ax*bz, nz=ax*by-ay*bx;
  const len=Math.sqrt(nx*nx+ny*ny+nz*nz)||1;
  outBuf.writeFloatLE(nx/len,off); outBuf.writeFloatLE(ny/len,off+4); outBuf.writeFloatLE(nz/len,off+8);
  outBuf.writeFloatLE(va[0],off+12); outBuf.writeFloatLE(va[1],off+16); outBuf.writeFloatLE(va[2],off+20);
  outBuf.writeFloatLE(vb[0],off+24); outBuf.writeFloatLE(vb[1],off+28); outBuf.writeFloatLE(vb[2],off+32);
  outBuf.writeFloatLE(vc[0],off+36); outBuf.writeFloatLE(vc[1],off+40); outBuf.writeFloatLE(vc[2],off+44);
  outBuf.writeUInt16LE(0, off+48);
}
fs.writeFileSync(outFile, outBuf);
console.log(`Written: ${outFile} (${(outBuf.length/1024/1024).toFixed(1)} MB)`);

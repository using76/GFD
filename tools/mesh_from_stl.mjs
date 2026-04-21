#!/usr/bin/env node
/**
 * GFD Mesh Generator v2 — STL 기반 Fluid/Solid mesh 생성
 *
 * Solid mesh: STL 삼각형을 직접 사용 (형상 그대로 표현)
 * Fluid mesh: 외부 hex grid (solid 영역 제외) + boundary 색상
 *
 * Usage: node tools/mesh_from_stl.mjs <stl_file> [--cell-size 0.04] [--padding 0.15]
 * Output: gui/public/mesh/mesh_fluid.vtk, mesh_solid.vtk
 */
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);

let stlFile = null;
let cellSize = 0.04;
let padding = 0.15;
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--cell-size') { cellSize = parseFloat(args[++i]); continue; }
  if (args[i] === '--padding') { padding = parseFloat(args[++i]); continue; }
  if (!stlFile) stlFile = args[i];
}
if (!stlFile) { console.error('Usage: node tools/mesh_from_stl.mjs <stl>'); process.exit(1); }

// ===== 1. Parse binary STL =====
console.log('=== 1. Parse STL ===');
const stlBuf = fs.readFileSync(stlFile);
const faceCount = stlBuf.readUInt32LE(80);
const verts = new Float32Array(faceCount * 9);
const normals = new Float32Array(faceCount * 3);
for (let i = 0; i < faceCount; i++) {
  const off = 84 + i * 50;
  normals[i*3]   = stlBuf.readFloatLE(off);
  normals[i*3+1] = stlBuf.readFloatLE(off+4);
  normals[i*3+2] = stlBuf.readFloatLE(off+8);
  for (let v = 0; v < 3; v++) {
    verts[i*9+v*3]   = stlBuf.readFloatLE(off+12+v*12);
    verts[i*9+v*3+1] = stlBuf.readFloatLE(off+16+v*12);
    verts[i*9+v*3+2] = stlBuf.readFloatLE(off+20+v*12);
  }
}
console.log(`  ${faceCount} triangles`);

// ===== 2. BBox, center, proportional scale =====
let mn = [Infinity,Infinity,Infinity], mx = [-Infinity,-Infinity,-Infinity];
for (let i = 0; i < verts.length; i += 3) {
  for (let d = 0; d < 3; d++) { if (verts[i+d]<mn[d]) mn[d]=verts[i+d]; if (verts[i+d]>mx[d]) mx[d]=verts[i+d]; }
}
const center = mn.map((m,i) => (m+mx[i])/2);
const spans = mn.map((m,i) => mx[i]-m);
const maxSpan = Math.max(...spans);
const scale = 2.0 / maxSpan;
console.log(`  Raw: [${mn.map(v=>v.toFixed(1))}] → [${mx.map(v=>v.toFixed(1))}]`);
console.log(`  Spans: ${spans.map(v=>v.toFixed(1)).join(' x ')}, scale=${scale.toExponential(3)}`);

// Scale
for (let i = 0; i < verts.length; i += 3) {
  verts[i]   = (verts[i]-center[0])*scale;
  verts[i+1] = (verts[i+1]-center[1])*scale;
  verts[i+2] = (verts[i+2]-center[2])*scale;
}

// Recompute bbox
mn = [Infinity,Infinity,Infinity]; mx = [-Infinity,-Infinity,-Infinity];
for (let i = 0; i < verts.length; i += 3) {
  for (let d = 0; d < 3; d++) { if (verts[i+d]<mn[d]) mn[d]=verts[i+d]; if (verts[i+d]>mx[d]) mx[d]=verts[i+d]; }
}
console.log(`  Scaled: [${mn.map(v=>v.toFixed(4))}] → [${mx.map(v=>v.toFixed(4))}]`);

// ===== 3. Per-triangle AABB =====
const triBB = new Float32Array(faceCount * 6);
for (let fi = 0; fi < faceCount; fi++) {
  let bx0=Infinity,by0=Infinity,bz0=Infinity,bx1=-Infinity,by1=-Infinity,bz1=-Infinity;
  for (let v = 0; v < 3; v++) {
    const x=verts[fi*9+v*3], y=verts[fi*9+v*3+1], z=verts[fi*9+v*3+2];
    if (x<bx0) bx0=x; if (x>bx1) bx1=x;
    if (y<by0) by0=y; if (y>by1) by1=y;
    if (z<bz0) bz0=z; if (z>bz1) bz1=z;
  }
  const b = fi*6;
  triBB[b]=bx0; triBB[b+1]=by0; triBB[b+2]=bz0; triBB[b+3]=bx1; triBB[b+4]=by1; triBB[b+5]=bz1;
}

// ===== 4. Grid =====
console.log('\n=== 2. Create grid ===');
const domMin = mn.map(v => v - padding);
const domMax = mx.map(v => v + padding);
const L = domMin.map((v,i) => domMax[i]-v);
const N = L.map(l => Math.max(4, Math.round(l / cellSize)));
const D = L.map((l,i) => l / N[i]);
const [nx,ny,nz] = N;
const [dx,dy,dz] = D;
const total = nx*ny*nz;
console.log(`  Domain: ${domMin.map(v=>v.toFixed(3))} → ${domMax.map(v=>v.toFixed(3))}`);
console.log(`  Grid: ${nx} x ${ny} x ${nz} = ${total} cells`);
console.log(`  Cell: ${dx.toFixed(4)} x ${dy.toFixed(4)} x ${dz.toFixed(4)}`);

// ===== 5. Ray-cast classification =====
console.log('\n=== 3. Ray-cast ===');
const cellType = new Uint8Array(total);
let nF=0, nS=0;
const t0 = Date.now();

function raycast(px, py, pz) {
  if (px<mn[0]||px>mx[0]||py<mn[1]||py>mx[1]||pz<mn[2]||pz>mx[2]) return false;
  let crossings = 0;
  for (let fi = 0; fi < faceCount; fi++) {
    const b=fi*6;
    if (py<triBB[b+1]||py>triBB[b+4]||pz<triBB[b+2]||pz>triBB[b+5]) continue;
    if (triBB[b+3]<px) continue;
    const i0=fi*9;
    const v0x=verts[i0],v0y=verts[i0+1],v0z=verts[i0+2];
    const e1x=verts[i0+3]-v0x, e1y=verts[i0+4]-v0y, e1z=verts[i0+5]-v0z;
    const e2x=verts[i0+6]-v0x, e2y=verts[i0+7]-v0y, e2z=verts[i0+8]-v0z;
    const hy=-e2z, hz=e2y;
    const a=e1y*hy+e1z*hz;
    if (a>-1e-10&&a<1e-10) continue;
    const f=1/a, sy=py-v0y, sz=pz-v0z;
    const u=f*(sy*hy+sz*hz);
    if (u<0||u>1) continue;
    const qx=sy*e1z-sz*e1y;
    const v=f*qx;
    if (v<0||u+v>1) continue;
    const t=f*(e2x*qx+e2y*(sz*e1x-(px-v0x)*e1z)+e2z*((px-v0x)*e1y-sy*e1x));
    if (t>1e-10) crossings++;
  }
  return crossings%2===1;
}

for (let k=0;k<nz;k++) {
  if (k%2===0) process.stdout.write(`  z=${k}/${nz}\r`);
  for (let j=0;j<ny;j++) for (let i=0;i<nx;i++) {
    const px=domMin[0]+(i+.5)*dx, py=domMin[1]+(j+.5)*dy, pz=domMin[2]+(k+.5)*dz;
    if (raycast(px,py,pz)) { cellType[k*ny*nx+j*nx+i]=1; nS++; } else nF++;
  }
}
console.log(`\n  ${((Date.now()-t0)/1000).toFixed(1)}s — Fluid: ${nF} (${(nF/total*100).toFixed(1)}%), Solid: ${nS} (${(nS/total*100).toFixed(1)}%)`);

// ===== 6. Fluid hex surface extraction =====
console.log('\n=== 4. Extract surfaces ===');
const COL = {
  inlet: [0.267,0.533,1.0], outlet: [1.0,0.267,0.267],
  wall: [0.267,0.8,0.267], iface: [1.0,0.4,0.133],
  solid: [0.6,0.6,0.65], solidIf: [1.0,0.45,0.15],
};

// Fluid: only domain boundary faces (no staircase at solid interface)
const fluidTris = [], fluidCols = [];
function addQ(arr, ca, x0,y0,z0,x1,y1,z1,x2,y2,z2,x3,y3,z3,c) {
  arr.push([x0,y0,z0,x1,y1,z1,x2,y2,z2],[x1,y1,z1,x3,y3,z3,x2,y2,z2]);
  ca.push(c,c);
}

for (let k=0;k<nz;k++) for (let j=0;j<ny;j++) for (let i=0;i<nx;i++) {
  if (cellType[k*ny*nx+j*nx+i]!==0) continue;
  const x0=domMin[0]+i*dx,x1=x0+dx,y0=domMin[1]+j*dy,y1=y0+dy,z0=domMin[2]+k*dz,z1=z0+dz;
  // Only emit domain boundary faces; at solid interface we'll use STL triangles instead
  if (i===0)    addQ(fluidTris,fluidCols, x0,y0,z0,x0,y1,z0,x0,y0,z1,x0,y1,z1, COL.inlet);
  if (i===nx-1) addQ(fluidTris,fluidCols, x1,y0,z0,x1,y0,z1,x1,y1,z0,x1,y1,z1, COL.outlet);
  if (j===0)    addQ(fluidTris,fluidCols, x0,y0,z0,x0,y0,z1,x1,y0,z0,x1,y0,z1, COL.wall);
  if (j===ny-1) addQ(fluidTris,fluidCols, x0,y1,z0,x1,y1,z0,x0,y1,z1,x1,y1,z1, COL.wall);
  if (k===0)    addQ(fluidTris,fluidCols, x0,y0,z0,x1,y0,z0,x0,y1,z0,x1,y1,z0, COL.wall);
  if (k===nz-1) addQ(fluidTris,fluidCols, x0,y0,z1,x0,y1,z1,x1,y0,z1,x1,y1,z1, COL.wall);
  // Solid interface: show staircase faces too for volume indication
  if (i>0    && cellType[k*ny*nx+j*nx+(i-1)]===1)   addQ(fluidTris,fluidCols, x0,y0,z0,x0,y1,z0,x0,y0,z1,x0,y1,z1, COL.iface);
  if (i<nx-1 && cellType[k*ny*nx+j*nx+(i+1)]===1)   addQ(fluidTris,fluidCols, x1,y0,z0,x1,y0,z1,x1,y1,z0,x1,y1,z1, COL.iface);
  if (j>0    && cellType[k*ny*nx+(j-1)*nx+i]===1)   addQ(fluidTris,fluidCols, x0,y0,z0,x0,y0,z1,x1,y0,z0,x1,y0,z1, COL.iface);
  if (j<ny-1 && cellType[k*ny*nx+(j+1)*nx+i]===1)   addQ(fluidTris,fluidCols, x0,y1,z0,x1,y1,z0,x0,y1,z1,x1,y1,z1, COL.iface);
  if (k>0    && cellType[(k-1)*ny*nx+j*nx+i]===1)   addQ(fluidTris,fluidCols, x0,y0,z0,x1,y0,z0,x0,y1,z0,x1,y1,z0, COL.iface);
  if (k<nz-1 && cellType[(k+1)*ny*nx+j*nx+i]===1)   addQ(fluidTris,fluidCols, x0,y0,z1,x0,y1,z1,x1,y0,z1,x1,y1,z1, COL.iface);
}

// Solid: use actual STL triangles (original geometry — not staircase!)
// Subsample for browser performance (max ~50K tris)
const maxSolidTris = 50000;
const solidStep = faceCount > maxSolidTris ? Math.ceil(faceCount / maxSolidTris) : 1;
const solidTris = [], solidCols = [];
for (let fi = 0; fi < faceCount; fi += solidStep) {
  const i = fi * 9;
  const nz_val = normals[fi*3+2];
  let col;
  if (nz_val > 0.5) col = [0.7, 0.72, 0.78];       // top face — light
  else if (nz_val < -0.5) col = [0.35, 0.36, 0.4];  // bottom face — dark
  else col = [0.55, 0.56, 0.62];                      // side face — medium
  solidTris.push([verts[i],verts[i+1],verts[i+2], verts[i+3],verts[i+4],verts[i+5], verts[i+6],verts[i+7],verts[i+8]]);
  solidCols.push(col);
}

console.log(`  Fluid: ${fluidTris.length} tris (hex boundary + interface)`);
console.log(`  Solid: ${solidTris.length} tris (original STL geometry)`);

// ===== 7. Write binary mesh data (direct meshDisplayData format) =====
console.log('\n=== 5. Write mesh.bin ===');
const outDir = path.join(__dirname, '..', 'gui', 'public', 'mesh');
fs.mkdirSync(outDir, { recursive: true });

function trisToArrays(tris, cols) {
  const pos = new Float32Array(tris.length * 9);
  const col = new Float32Array(tris.length * 9);
  const wire = new Float32Array(tris.length * 18); // 3 edges * 2 points * 3 coords
  for (let i = 0; i < tris.length; i++) {
    const t = tris[i], c = cols[i];
    for (let j = 0; j < 9; j++) pos[i*9+j] = t[j];
    for (let v = 0; v < 3; v++) { col[i*9+v*3]=c[0]; col[i*9+v*3+1]=c[1]; col[i*9+v*3+2]=c[2]; }
    // 3 edge line segments
    wire[i*18]   =t[0]; wire[i*18+1] =t[1]; wire[i*18+2] =t[2];
    wire[i*18+3] =t[3]; wire[i*18+4] =t[4]; wire[i*18+5] =t[5];
    wire[i*18+6] =t[3]; wire[i*18+7] =t[4]; wire[i*18+8] =t[5];
    wire[i*18+9] =t[6]; wire[i*18+10]=t[7]; wire[i*18+11]=t[8];
    wire[i*18+12]=t[6]; wire[i*18+13]=t[7]; wire[i*18+14]=t[8];
    wire[i*18+15]=t[0]; wire[i*18+16]=t[1]; wire[i*18+17]=t[2];
  }
  return { pos, col, wire };
}

const fluid = trisToArrays(fluidTris, fluidCols);
const solid = trisToArrays(solidTris, solidCols);

// Binary format: [header 32 bytes] [fluid.pos] [fluid.col] [fluid.wire] [solid.pos] [solid.col] [solid.wire]
// Header: magic(4) fluidTris(4) solidTris(4) nx(4) ny(4) nz(4) nFluid(4) nSolid(4)
const header = Buffer.alloc(32);
header.write('GFDM', 0);
header.writeUInt32LE(fluidTris.length, 4);
header.writeUInt32LE(solidTris.length, 8);
header.writeUInt32LE(nx, 12);
header.writeUInt32LE(ny, 16);
header.writeUInt32LE(nz, 20);
header.writeUInt32LE(nF, 24);
header.writeUInt32LE(nS, 28);

const parts = [header,
  Buffer.from(fluid.pos.buffer), Buffer.from(fluid.col.buffer), Buffer.from(fluid.wire.buffer),
  Buffer.from(solid.pos.buffer), Buffer.from(solid.col.buffer), Buffer.from(solid.wire.buffer),
];
const binPath = path.join(outDir, 'mesh.bin');
fs.writeFileSync(binPath, Buffer.concat(parts));
const mb = (parts.reduce((s,p)=>s+p.length,0)/1024/1024).toFixed(1);
console.log(`  ${binPath}: ${mb} MB`);
console.log(`  Fluid: ${fluidTris.length} tris, Solid: ${solidTris.length} tris`);

console.log(`\n✅ Done! Grid ${nx}x${ny}x${nz}, Fluid ${nF}, Solid ${nS}`);
console.log(`   Refresh http://localhost:5175 to view`);

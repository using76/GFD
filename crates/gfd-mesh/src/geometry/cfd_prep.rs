//! CFD-specific geometry preparation operations.
//!
//! Provides enclosure creation, fluid region extraction, region naming,
//! symmetry cutting, and surface wrapping for CFD simulation setup.

use crate::geometry::distance_field::Triangle;
use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};
use std::collections::HashMap;

fn sub3(a: [f64;3], b: [f64;3]) -> [f64;3] { [a[0]-b[0],a[1]-b[1],a[2]-b[2]] }
fn dot3(a: [f64;3], b: [f64;3]) -> f64 { a[0]*b[0]+a[1]*b[1]+a[2]*b[2] }
fn length3(a: [f64;3]) -> f64 { dot3(a,a).sqrt() }
fn normalize3(a: [f64;3]) -> [f64;3] {
    let l = length3(a); if l < 1e-30 { [0.0,0.0,0.0] } else { [a[0]/l,a[1]/l,a[2]/l] }
}
fn cross3(a: [f64;3], b: [f64;3]) -> [f64;3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}

/// An enclosure mesh with named boundary regions.
#[derive(Debug, Clone)]
pub struct EnclosureMesh {
    pub mesh: UnstructuredMesh,
    pub inlet_faces: Vec<usize>,
    pub outlet_faces: Vec<usize>,
    pub wall_faces: Vec<usize>,
}

/// Create an enclosure (bounding-box domain) around a geometry.
/// `padding` is `[+x, -x, +y, -y, +z, -z]`.
pub fn create_enclosure(geometry_bbox: [[f64;3];2], padding: [f64;6]) -> EnclosureMesh {
    let xn = geometry_bbox[0][0] - padding[1];
    let xx = geometry_bbox[1][0] + padding[0];
    let yn = geometry_bbox[0][1] - padding[3];
    let yx = geometry_bbox[1][1] + padding[2];
    let zn = geometry_bbox[0][2] - padding[5];
    let zx = geometry_bbox[1][2] + padding[4];
    let nodes = vec![
        Node::new(0,[xn,yn,zn]), Node::new(1,[xx,yn,zn]),
        Node::new(2,[xx,yx,zn]), Node::new(3,[xn,yx,zn]),
        Node::new(4,[xn,yn,zx]), Node::new(5,[xx,yn,zx]),
        Node::new(6,[xx,yx,zx]), Node::new(7,[xn,yx,zx]),
    ];
    let center = [(xn+xx)*0.5,(yn+yx)*0.5,(zn+zx)*0.5];
    let volume = (xx-xn)*(yx-yn)*(zx-zn);
    let face_defs: Vec<(Vec<usize>,[f64;3],&str)> = vec![
        (vec![0,3,7,4],[-1.0,0.0,0.0],"inlet"),
        (vec![1,2,6,5],[1.0,0.0,0.0],"outlet"),
        (vec![0,1,5,4],[0.0,-1.0,0.0],"wall"),
        (vec![3,2,6,7],[0.0,1.0,0.0],"wall"),
        (vec![0,1,2,3],[0.0,0.0,-1.0],"wall"),
        (vec![4,5,6,7],[0.0,0.0,1.0],"wall"),
    ];
    let mut faces = Vec::new();
    let mut inlet_faces = Vec::new();
    let mut outlet_faces = Vec::new();
    let mut wall_faces = Vec::new();
    let mut fids = Vec::new();
    for (i,(fnodes,normal,label)) in face_defs.iter().enumerate() {
        let fc = face_center(&nodes, fnodes);
        let area = face_area(&nodes, fnodes);
        faces.push(Face::new(i, fnodes.clone(), 0, None, area, *normal, fc));
        fids.push(i);
        match *label { "inlet" => inlet_faces.push(i), "outlet" => outlet_faces.push(i), _ => wall_faces.push(i) }
    }
    let cell = Cell::new(0, (0..8).collect(), fids, volume, center);
    let mut patches = Vec::new();
    if !inlet_faces.is_empty() { patches.push(BoundaryPatch::new("inlet", inlet_faces.clone())); }
    if !outlet_faces.is_empty() { patches.push(BoundaryPatch::new("outlet", outlet_faces.clone())); }
    if !wall_faces.is_empty() { patches.push(BoundaryPatch::new("wall", wall_faces.clone())); }
    let mesh = UnstructuredMesh::from_components(nodes, faces, vec![cell], patches);
    EnclosureMesh { mesh, inlet_faces, outlet_faces, wall_faces }
}

fn face_center(nodes: &[Node], nids: &[usize]) -> [f64;3] {
    let n = nids.len() as f64;
    let mut c = [0.0;3];
    for &nid in nids { let p = nodes[nid].position; c[0]+=p[0]; c[1]+=p[1]; c[2]+=p[2]; }
    [c[0]/n, c[1]/n, c[2]/n]
}

fn face_area(nodes: &[Node], nids: &[usize]) -> f64 {
    if nids.len() < 3 { return 0.0; }
    let n = nids.len();
    let (mut nx,mut ny,mut nz) = (0.0f64,0.0f64,0.0f64);
    for i in 0..n {
        let p1 = nodes[nids[i]].position; let p2 = nodes[nids[(i+1)%n]].position;
        nx += (p1[1]-p2[1])*(p1[2]+p2[2]);
        ny += (p1[2]-p2[2])*(p1[0]+p2[0]);
        nz += (p1[0]-p2[0])*(p1[1]+p2[1]);
    }
    0.5*(nx*nx+ny*ny+nz*nz).sqrt()
}

fn face_normal_from_nodes(nodes: &[Node], nids: &[usize]) -> [f64;3] {
    if nids.len() < 3 { return [0.0,0.0,1.0]; }
    let p0 = nodes[nids[0]].position;
    let p1 = nodes[nids[1]].position;
    let p2 = nodes[nids[2]].position;
    normalize3(cross3(sub3(p1,p0), sub3(p2,p0)))
}

fn face_diagonal(nodes: &[Node], nids: &[usize]) -> f64 {
    let mut mx = 0.0f64;
    for i in 0..nids.len() {
        for j in (i+1)..nids.len() {
            let d = length3(sub3(nodes[nids[i]].position, nodes[nids[j]].position));
            if d > mx { mx = d; }
        }
    }
    mx
}

fn rebuild_faces_generic(
    nodes: &[Node], cells: &mut Vec<Cell>,
) -> (Vec<Face>, Vec<usize>) {
    let mut fmap: HashMap<Vec<usize>,(usize,Vec<usize>)> = HashMap::new();
    let mut faces: Vec<Face> = Vec::new();
    let mut bfids: Vec<usize> = Vec::new();
    let cfn: Vec<Vec<Vec<usize>>> = cells.iter().map(|cell| {
        let cn = &cell.nodes;
        match cn.len() {
            8 => vec![
                vec![cn[0],cn[1],cn[2],cn[3]], vec![cn[4],cn[5],cn[6],cn[7]],
                vec![cn[0],cn[1],cn[5],cn[4]], vec![cn[3],cn[2],cn[6],cn[7]],
                vec![cn[0],cn[3],cn[7],cn[4]], vec![cn[1],cn[2],cn[6],cn[5]],
            ],
            4 => vec![
                vec![cn[0],cn[1],cn[2]], vec![cn[0],cn[1],cn[3]],
                vec![cn[0],cn[2],cn[3]], vec![cn[1],cn[2],cn[3]],
            ],
            _ => vec![],
        }
    }).collect();
    for (ci, fl) in cfn.iter().enumerate() {
        for fn_nodes in fl {
            let mut sorted = fn_nodes.clone(); sorted.sort();
            if let Some((oci, _)) = fmap.get(&sorted) {
                let fid = faces.len();
                let fc = face_center(nodes, fn_nodes);
                let area = face_area(nodes, fn_nodes);
                let normal = face_normal_from_nodes(nodes, fn_nodes);
                faces.push(Face::new(fid, fn_nodes.clone(), *oci, Some(ci), area, normal, fc));
                cells[*oci].faces.push(fid); cells[ci].faces.push(fid);
                fmap.remove(&sorted);
            } else {
                fmap.insert(sorted, (ci, fn_nodes.clone()));
            }
        }
    }
    for (_, (oci, fn_nodes)) in &fmap {
        let fid = faces.len();
        let fc = face_center(nodes, fn_nodes);
        let area = face_area(nodes, fn_nodes);
        let normal = face_normal_from_nodes(nodes, fn_nodes);
        faces.push(Face::new(fid, fn_nodes.clone(), *oci, None, area, normal, fc));
        cells[*oci].faces.push(fid);
        bfids.push(fid);
    }
    (faces, bfids)
}

/// Extract fluid region by removing cells inside the solid (SDF < 0).
pub fn extract_fluid_region(
    enclosure: &UnstructuredMesh,
    solid_sdf: &dyn Fn([f64;3]) -> f64,
) -> UnstructuredMesh {
    let mut new_cells: Vec<Cell> = Vec::new();
    for cell in &enclosure.cells {
        if solid_sdf(cell.center) >= 0.0 {
            let nid = new_cells.len();
            new_cells.push(Cell::new(nid, cell.nodes.clone(), Vec::new(), cell.volume, cell.center));
        }
    }
    let nodes = enclosure.nodes.clone();
    let (faces, bfids) = rebuild_faces_generic(&nodes, &mut new_cells);
    let mut patches = Vec::new();
    if !bfids.is_empty() { patches.push(BoundaryPatch::new("boundary", bfids)); }
    UnstructuredMesh::from_components(nodes, faces, new_cells, patches)
}

/// Name mesh boundary regions by face normal direction.
pub fn name_regions_by_normal(
    mesh: &UnstructuredMesh, angle_threshold_deg: f64,
) -> HashMap<String, Vec<usize>> {
    let cos_th = angle_threshold_deg.to_radians().cos();
    let dirs: [(&str,[f64;3]);6] = [
        ("pos_x",[1.0,0.0,0.0]),("neg_x",[-1.0,0.0,0.0]),
        ("pos_y",[0.0,1.0,0.0]),("neg_y",[0.0,-1.0,0.0]),
        ("pos_z",[0.0,0.0,1.0]),("neg_z",[0.0,0.0,-1.0]),
    ];
    let mut regions: HashMap<String,Vec<usize>> = HashMap::new();
    for face in &mesh.faces {
        if face.neighbor_cell.is_some() { continue; }
        let n = normalize3(face.normal);
        let mut best_name = "other"; let mut best_cos = -1.0f64;
        for (name,dir) in &dirs {
            let c = dot3(n,*dir);
            if c > best_cos { best_cos = c; best_name = name; }
        }
        if best_cos >= cos_th {
            regions.entry(best_name.to_string()).or_default().push(face.id);
        } else {
            regions.entry("other".to_string()).or_default().push(face.id);
        }
    }
    regions
}

/// Symmetry cut: keep cells on positive side of plane.
pub fn symmetry_cut(mesh: &UnstructuredMesh, normal: [f64;3], offset: f64) -> UnstructuredMesh {
    let n = normalize3(normal);
    let mut kept: Vec<Cell> = Vec::new();
    for cell in &mesh.cells {
        if dot3(n, cell.center) - offset >= 0.0 {
            let nid = kept.len();
            kept.push(Cell::new(nid, cell.nodes.clone(), Vec::new(), cell.volume, cell.center));
        }
    }
    let nodes = mesh.nodes.clone();
    let (faces, bfids) = rebuild_faces_generic(&nodes, &mut kept);
    let mut sym_fids = Vec::new();
    let mut other_fids = Vec::new();
    for &fid in &bfids {
        let fc = faces[fid].center;
        let d = (dot3(n, fc) - offset).abs();
        let diag = face_diagonal(&nodes, &faces[fid].nodes);
        if d < 0.1 * diag.max(1e-10) { sym_fids.push(fid); } else { other_fids.push(fid); }
    }
    let mut patches = Vec::new();
    if !sym_fids.is_empty() { patches.push(BoundaryPatch::new("symmetry", sym_fids)); }
    if !other_fids.is_empty() { patches.push(BoundaryPatch::new("boundary", other_fids)); }
    UnstructuredMesh::from_components(nodes, faces, kept, patches)
}

/// Surface wrap: create a simplified closed surface around complex geometry.
pub fn surface_wrap(triangles: &[Triangle], cell_size: f64) -> Vec<Triangle> {
    if triangles.is_empty() || cell_size <= 0.0 { return Vec::new(); }
    let mut bmin = [f64::MAX;3]; let mut bmax = [f64::MIN;3];
    for tri in triangles {
        for v in &[tri.v0,tri.v1,tri.v2] {
            for d in 0..3 { bmin[d]=bmin[d].min(v[d]); bmax[d]=bmax[d].max(v[d]); }
        }
    }
    let pad = cell_size;
    for d in 0..3 { bmin[d]-=pad; bmax[d]+=pad; }
    let nx = ((bmax[0]-bmin[0])/cell_size).ceil().max(1.0).min(256.0) as usize;
    let ny = ((bmax[1]-bmin[1])/cell_size).ceil().max(1.0).min(256.0) as usize;
    let nz = ((bmax[2]-bmin[2])/cell_size).ceil().max(1.0).min(256.0) as usize;
    let total = nx*ny*nz;
    let mut occupied = vec![false; total];
    for tri in triangles {
        let mut tmn = [f64::MAX;3]; let mut tmx = [f64::MIN;3];
        for v in &[tri.v0,tri.v1,tri.v2] {
            for d in 0..3 { tmn[d]=tmn[d].min(v[d]); tmx[d]=tmx[d].max(v[d]); }
        }
        let i0 = ((tmn[0]-bmin[0])/cell_size).floor() as i64;
        let i1 = ((tmx[0]-bmin[0])/cell_size).ceil() as i64;
        let j0 = ((tmn[1]-bmin[1])/cell_size).floor() as i64;
        let j1 = ((tmx[1]-bmin[1])/cell_size).ceil() as i64;
        let k0 = ((tmn[2]-bmin[2])/cell_size).floor() as i64;
        let k1 = ((tmx[2]-bmin[2])/cell_size).ceil() as i64;
        for i in i0.max(0)..=i1.min(nx as i64-1) {
            for j in j0.max(0)..=j1.min(ny as i64-1) {
                for k in k0.max(0)..=k1.min(nz as i64-1) {
                    let idx = i as usize*ny*nz + j as usize*nz + k as usize;
                    if idx < total { occupied[idx] = true; }
                }
            }
        }
    }
    // Flood-fill exterior
    let mut exterior = vec![false; total];
    let mut stack: Vec<(usize,usize,usize)> = Vec::new();
    for i in 0..nx { for j in 0..ny { for k in 0..nz {
        if i==0||i==nx-1||j==0||j==ny-1||k==0||k==nz-1 {
            let idx = i*ny*nz+j*nz+k;
            if !occupied[idx] && !exterior[idx] { exterior[idx]=true; stack.push((i,j,k)); }
        }
    }}}
    while let Some((i,j,k)) = stack.pop() {
        for (di,dj,dk) in &[(1i64,0,0),(-1,0,0),(0,1,0),(0,-1,0),(0,0,1),(0,0,-1)] {
            let ni = i as i64+di; let nj = j as i64+dj; let nk = k as i64+dk;
            if ni>=0 && ni<nx as i64 && nj>=0 && nj<ny as i64 && nk>=0 && nk<nz as i64 {
                let idx = ni as usize*ny*nz + nj as usize*nz + nk as usize;
                if !occupied[idx] && !exterior[idx] { exterior[idx]=true; stack.push((ni as usize,nj as usize,nk as usize)); }
            }
        }
    }
    let mut solid = vec![false; total];
    for idx in 0..total { solid[idx] = occupied[idx] || !exterior[idx]; }
    let mut result = Vec::new();
    for i in 0..nx { for j in 0..ny { for k in 0..nz {
        let idx = i*ny*nz+j*nz+k;
        if !solid[idx] { continue; }
        let x0 = bmin[0]+i as f64*cell_size; let y0 = bmin[1]+j as f64*cell_size; let z0 = bmin[2]+k as f64*cell_size;
        let x1 = x0+cell_size; let y1 = y0+cell_size; let z1 = z0+cell_size;
        let nb: [(i64,i64,i64,[[f64;3];4]);6] = [
            (1,0,0,[[x1,y0,z0],[x1,y1,z0],[x1,y1,z1],[x1,y0,z1]]),
            (-1,0,0,[[x0,y0,z0],[x0,y0,z1],[x0,y1,z1],[x0,y1,z0]]),
            (0,1,0,[[x0,y1,z0],[x0,y1,z1],[x1,y1,z1],[x1,y1,z0]]),
            (0,-1,0,[[x0,y0,z0],[x1,y0,z0],[x1,y0,z1],[x0,y0,z1]]),
            (0,0,1,[[x0,y0,z1],[x1,y0,z1],[x1,y1,z1],[x0,y1,z1]]),
            (0,0,-1,[[x0,y0,z0],[x0,y1,z0],[x1,y1,z0],[x1,y0,z0]]),
        ];
        for (di,dj,dk,quad) in &nb {
            let ni = i as i64+di; let nj = j as i64+dj; let nk = k as i64+dk;
            let ns = if ni>=0 && ni<nx as i64 && nj>=0 && nj<ny as i64 && nk>=0 && nk<nz as i64 {
                solid[ni as usize*ny*nz+nj as usize*nz+nk as usize]
            } else { false };
            if !ns {
                result.push(Triangle::new(quad[0],quad[1],quad[2]));
                result.push(Triangle::new(quad[0],quad[2],quad[3]));
            }
        }
    }}}
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test] fn test_create_enclosure() {
        let enc = create_enclosure([[0.0,0.0,0.0],[1.0,1.0,1.0]], [2.0;6]);
        assert_eq!(enc.mesh.cells.len(), 1);
        assert_eq!(enc.mesh.nodes.len(), 8);
        assert_eq!(enc.mesh.faces.len(), 6);
        assert_eq!(enc.inlet_faces.len(), 1);
        assert_eq!(enc.outlet_faces.len(), 1);
        assert_eq!(enc.wall_faces.len(), 4);
    }
    #[test] fn test_enclosure_volume() {
        let enc = create_enclosure([[0.0,0.0,0.0],[1.0,1.0,1.0]], [1.0;6]);
        let vol: f64 = enc.mesh.cells.iter().map(|c| c.volume).sum();
        assert!((vol - 27.0).abs() < 1e-10);
    }
    #[test] fn test_extract_fluid() {
        let mesh = StructuredMesh::uniform(4,4,1,4.0,4.0,1.0).to_unstructured();
        let sdf = |p: [f64;3]| { let d=p[0]-2.0; let e=p[1]-2.0; let f=p[2]-0.5; (d*d+e*e+f*f).sqrt()-1.0 };
        let fluid = extract_fluid_region(&mesh, &sdf);
        assert!(fluid.cells.len() < mesh.cells.len());
        assert!(fluid.cells.len() > 0);
    }
    #[test] fn test_extract_no_solid() {
        let mesh = StructuredMesh::uniform(2,2,1,2.0,2.0,1.0).to_unstructured();
        let sdf = |_p: [f64;3]| 10.0;
        assert_eq!(extract_fluid_region(&mesh, &sdf).cells.len(), mesh.cells.len());
    }
    #[test] fn test_name_regions() {
        let mesh = StructuredMesh::uniform(2,2,1,2.0,2.0,1.0).to_unstructured();
        let r = name_regions_by_normal(&mesh, 45.0);
        assert!(!r.is_empty());
    }
    #[test] fn test_symmetry_cut() {
        let mesh = StructuredMesh::uniform(4,2,1,4.0,2.0,1.0).to_unstructured();
        let nc = mesh.cells.len();
        let cut = symmetry_cut(&mesh, [1.0,0.0,0.0], 2.0);
        assert!(cut.cells.len() < nc);
        assert!(cut.cells.len() > 0);
    }
    #[test] fn test_symmetry_keep_all() {
        let mesh = StructuredMesh::uniform(2,2,1,2.0,2.0,1.0).to_unstructured();
        assert_eq!(symmetry_cut(&mesh, [0.0,0.0,1.0], -10.0).cells.len(), mesh.cells.len());
    }
    #[test] fn test_surface_wrap() {
        let tris = vec![
            Triangle::new([0.0,0.0,0.0],[1.0,0.0,0.0],[1.0,1.0,0.0]),
            Triangle::new([0.0,0.0,0.0],[1.0,1.0,0.0],[0.0,1.0,0.0]),
            Triangle::new([0.0,0.0,1.0],[1.0,0.0,1.0],[1.0,1.0,1.0]),
            Triangle::new([0.0,0.0,1.0],[1.0,1.0,1.0],[0.0,1.0,1.0]),
        ];
        assert!(!surface_wrap(&tris, 0.5).is_empty());
    }
    #[test] fn test_surface_wrap_empty() { assert!(surface_wrap(&[], 0.5).is_empty()); }
}

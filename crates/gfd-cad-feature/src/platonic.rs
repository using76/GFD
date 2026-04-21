//! Platonic solids — canonical regular polyhedra. Each builder creates
//! vertices + triangle faces directly in the arena (no pad/revolve),
//! wrapped as a `Solid` via a single `Shell`.

use gfd_cad_geom::{GeomError, Point3, surface::Plane, Direction3};
use gfd_cad_topo::{
    builder::{make_line_edge, make_wire},
    shape::{Shape, SurfaceGeom},
    Orientation, ShapeArena, ShapeId, TopoError, TopoResult,
};

/// Build a solid from a list of vertex positions + polygon face indices
/// (each face is a CCW ring of vertex indices). Vertices/edges are
/// de-duplicated per adjacency pair.
fn build_faceted_solid(
    arena: &mut ShapeArena,
    verts: &[Point3],
    faces: &[Vec<usize>],
) -> TopoResult<ShapeId> {
    if verts.len() < 4 || faces.len() < 4 {
        return Err(TopoError::Geom(GeomError::Degenerate("polyhedron: too few vertices/faces")));
    }
    let vert_points: Vec<Point3> = verts.to_vec();
    let default_plane = Plane::new(Point3::ORIGIN, Direction3::Z, Direction3::X);
    let mut face_ids: Vec<ShapeId> = Vec::with_capacity(faces.len());
    for face in faces {
        let n = face.len();
        if n < 3 {
            return Err(TopoError::Geom(GeomError::Degenerate("polyhedron: face < 3 verts")));
        }
        let mut edge_ids: Vec<(ShapeId, Orientation)> = Vec::with_capacity(n);
        for k in 0..n {
            let a = vert_points[face[k]];
            let b = vert_points[face[(k + 1) % n]];
            let e = make_line_edge(arena, a, b)?;
            edge_ids.push((e, Orientation::Forward));
        }
        let wire = make_wire(arena, edge_ids);
        let face_id = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(default_plane),
            wires: vec![wire],
            orient: Orientation::Forward,
        });
        face_ids.push(face_id);
    }
    let shell = arena.push(Shape::Shell {
        faces: face_ids.into_iter().map(|id| (id, Orientation::Forward)).collect(),
    });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Regular tetrahedron centred at the origin, vertices on a unit sphere
/// (scaled by `scale`). 4 vertices + 4 triangle faces.
pub fn tetrahedron_solid(arena: &mut ShapeArena, scale: f64) -> TopoResult<ShapeId> {
    if scale <= 0.0 {
        return Err(TopoError::Geom(GeomError::Degenerate("tetrahedron: scale must be > 0")));
    }
    let s = scale;
    // Standard coords: permutations of (±1, ±1, ±1) with even parity.
    let v: Vec<Point3> = [
        ( 1.0,  1.0,  1.0),
        (-1.0, -1.0,  1.0),
        (-1.0,  1.0, -1.0),
        ( 1.0, -1.0, -1.0),
    ].iter().map(|(x, y, z)| Point3::new(x * s, y * s, z * s)).collect();
    // CCW-oriented outward faces.
    let faces = vec![
        vec![0, 1, 2],
        vec![0, 3, 1],
        vec![0, 2, 3],
        vec![1, 3, 2],
    ];
    build_faceted_solid(arena, &v, &faces)
}

/// Icosphere — start with a regular icosahedron and subdivide each
/// triangle `subdivisions` times, projecting every new vertex onto the
/// sphere of radius `radius`. Produces much rounder triangle meshes than
/// a UV-sphere with the same triangle count.
pub fn icosphere_solid(
    arena: &mut ShapeArena,
    radius: f64,
    subdivisions: usize,
) -> TopoResult<ShapeId> {
    if radius <= 0.0 {
        return Err(TopoError::Geom(GeomError::Degenerate("icosphere: radius must be > 0")));
    }
    // Start with icosahedron vertices + faces on unit sphere.
    let phi = (1.0 + 5.0_f64.sqrt()) * 0.5;
    let norm = (1.0 + phi * phi).sqrt();
    let mut verts: Vec<Point3> = [
        ( 0.0,  1.0,  phi), ( 0.0, -1.0,  phi), ( 0.0,  1.0, -phi), ( 0.0, -1.0, -phi),
        ( 1.0,  phi,  0.0), (-1.0,  phi,  0.0), ( 1.0, -phi,  0.0), (-1.0, -phi,  0.0),
        ( phi,  0.0,  1.0), ( phi,  0.0, -1.0), (-phi,  0.0,  1.0), (-phi,  0.0, -1.0),
    ].iter().map(|(x, y, z)| Point3::new(x / norm, y / norm, z / norm)).collect();
    let mut faces: Vec<[usize; 3]> = vec![
        [0, 1, 8], [0, 8, 4], [0, 4, 5], [0, 5, 10], [0, 10, 1],
        [1, 6, 8], [8, 6, 9], [8, 9, 4], [4, 9, 2], [4, 2, 5],
        [5, 2, 11], [5, 11, 10], [10, 11, 7], [10, 7, 1], [1, 7, 6],
        [3, 9, 6], [3, 2, 9], [3, 11, 2], [3, 7, 11], [3, 6, 7],
    ];
    for _ in 0..subdivisions {
        let mut new_faces: Vec<[usize; 3]> = Vec::with_capacity(faces.len() * 4);
        let mut mid_cache: std::collections::HashMap<(usize, usize), usize> = std::collections::HashMap::new();
        let mut make_mid = |a: usize, b: usize, vs: &mut Vec<Point3>| -> usize {
            let k = if a < b { (a, b) } else { (b, a) };
            if let Some(&i) = mid_cache.get(&k) { return i; }
            let pa = vs[a];
            let pb = vs[b];
            let mx = 0.5 * (pa.x + pb.x);
            let my = 0.5 * (pa.y + pb.y);
            let mz = 0.5 * (pa.z + pb.z);
            let ml = (mx*mx + my*my + mz*mz).sqrt();
            vs.push(Point3::new(mx / ml, my / ml, mz / ml));
            let i = vs.len() - 1;
            mid_cache.insert(k, i);
            i
        };
        for f in &faces {
            let a = f[0]; let b = f[1]; let c = f[2];
            let ab = make_mid(a, b, &mut verts);
            let bc = make_mid(b, c, &mut verts);
            let ca = make_mid(c, a, &mut verts);
            new_faces.push([a, ab, ca]);
            new_faces.push([b, bc, ab]);
            new_faces.push([c, ca, bc]);
            new_faces.push([ab, bc, ca]);
        }
        faces = new_faces;
    }
    // Scale to requested radius.
    let scaled: Vec<Point3> = verts.iter().map(|p|
        Point3::new(p.x * radius, p.y * radius, p.z * radius)
    ).collect();
    let face_vecs: Vec<Vec<usize>> = faces.iter().map(|f| vec![f[0], f[1], f[2]]).collect();
    build_faceted_solid(arena, &scaled, &face_vecs)
}

/// Regular icosahedron centred at the origin, vertices scaled so that
/// the bounding sphere radius = `scale`. 12 vertices, 20 triangular faces.
/// Uses the golden-ratio parameterisation: vertices at (0, ±1, ±φ),
/// (±1, ±φ, 0), (±φ, 0, ±1).
pub fn icosahedron_solid(arena: &mut ShapeArena, scale: f64) -> TopoResult<ShapeId> {
    if scale <= 0.0 {
        return Err(TopoError::Geom(GeomError::Degenerate("icosahedron: scale must be > 0")));
    }
    let phi = (1.0 + 5.0_f64.sqrt()) * 0.5;
    // Normalise so bounding radius = 1, then scale.
    let r = (1.0 + phi * phi).sqrt();
    let n = scale / r;
    let v: Vec<Point3> = [
        ( 0.0,  1.0,  phi), ( 0.0, -1.0,  phi), ( 0.0,  1.0, -phi), ( 0.0, -1.0, -phi),
        ( 1.0,  phi,  0.0), (-1.0,  phi,  0.0), ( 1.0, -phi,  0.0), (-1.0, -phi,  0.0),
        ( phi,  0.0,  1.0), ( phi,  0.0, -1.0), (-phi,  0.0,  1.0), (-phi,  0.0, -1.0),
    ].iter().map(|(x, y, z)| Point3::new(x * n, y * n, z * n)).collect();
    // 20 CCW-oriented outward faces (standard icosahedron triangulation).
    let faces = vec![
        vec![0,  1,  8], vec![0,  8,  4], vec![0,  4,  5], vec![0,  5, 10], vec![0, 10,  1],
        vec![1,  6,  8], vec![8,  6,  9], vec![8,  9,  4], vec![4,  9,  2], vec![4,  2,  5],
        vec![5,  2, 11], vec![5, 11, 10], vec![10, 11,  7], vec![10,  7,  1], vec![1,  7,  6],
        vec![3,  9,  6], vec![3,  2,  9], vec![3, 11,  2], vec![3,  7, 11], vec![3,  6,  7],
    ];
    build_faceted_solid(arena, &v, &faces)
}

/// Regular octahedron centred at the origin, vertices on the ±axes at
/// distance `scale`. 6 vertices + 8 triangle faces.
pub fn octahedron_solid(arena: &mut ShapeArena, scale: f64) -> TopoResult<ShapeId> {
    if scale <= 0.0 {
        return Err(TopoError::Geom(GeomError::Degenerate("octahedron: scale must be > 0")));
    }
    let s = scale;
    let v: Vec<Point3> = vec![
        Point3::new( s,  0.0,  0.0),  // 0 +X
        Point3::new(-s,  0.0,  0.0),  // 1 -X
        Point3::new( 0.0,  s,  0.0),  // 2 +Y
        Point3::new( 0.0, -s,  0.0),  // 3 -Y
        Point3::new( 0.0,  0.0,  s),  // 4 +Z
        Point3::new( 0.0,  0.0, -s),  // 5 -Z
    ];
    // 8 faces, each a triangle made of one vertex from {±X}, {±Y}, {±Z}.
    let faces = vec![
        vec![0, 2, 4], vec![2, 1, 4], vec![1, 3, 4], vec![3, 0, 4], // +Z cap
        vec![2, 0, 5], vec![1, 2, 5], vec![3, 1, 5], vec![0, 3, 5], // -Z cap
    ];
    build_faceted_solid(arena, &v, &faces)
}

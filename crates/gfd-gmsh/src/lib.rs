//! # gfd-gmsh
//!
//! Gmsh backend for **CAD-aware CFD prep**, using Gmsh's built-in OpenCASCADE
//! kernel. One library covers the whole tail the AI needs:
//!
//! * primitives — `occ.addBox/Sphere/Cylinder/Cone`
//! * **boolean / enclosure** — `occ.cut` (enclosure − solid = fluid), `occ.fuse`
//! * **shape healing** — `occ.healShapes` (degenerate/small edges & faces, sew)
//! * **meshing** — surface ([`GmshSession::tessellate`], for the viewport) and
//!   volume ([`GmshSession::volume_mesh`], tets for the solver)
//!
//! ## Feature flags (mirrors `gfd-gpu`'s optional `cuda`)
//!
//! * `gmsh` — links the Gmsh SDK (`libgmsh` via `GMSH_LIB_DIR`, see `build.rs`).
//!
//! > Licensing: Gmsh is GPL-2+. Linking `libgmsh` makes the distributed product
//! > GPL-encumbered (a release-time decision — see the project plan).
//!
//! Without the `gmsh` feature the crate compiles as a thin stub so the workspace
//! builds with no Gmsh SDK; operations return [`GmshError::NotBuilt`] and callers
//! fall back to the pure-Rust `gfd-mesh`/`gfd-cad`.
//!
//! Gmsh has a single, non-thread-safe global context, so [`GmshSession`] holds a
//! process-global lock for its lifetime — create one at a time.

use thiserror::Error;

/// Errors from the Gmsh backend.
#[derive(Debug, Error)]
pub enum GmshError {
    /// The crate was compiled without the `gmsh` feature (Gmsh not linked).
    #[error("gfd-gmsh was built without the `gmsh` feature — Gmsh is not linked")]
    NotBuilt,
    /// A Gmsh API call returned a non-zero error code.
    #[error("Gmsh operation failed: {0}")]
    Failed(String),
}

/// `true` when this crate was compiled with the `gmsh` feature (Gmsh linked).
pub const fn is_available() -> bool {
    cfg!(feature = "gmsh")
}

/// Coarse statistics from a mesh run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshStats {
    /// Number of mesh nodes.
    pub nodes: usize,
    /// Length of the flat coordinate array (== nodes * 3).
    pub coord_len: usize,
}

#[cfg(not(feature = "gmsh"))]
mod backend {
    use super::{GmshError, MeshStats};
    /// Stub: meshing requires the `gmsh` feature.
    pub fn mesh_box(_dx: f64, _dy: f64, _dz: f64, _mesh_size: f64) -> Result<MeshStats, GmshError> {
        Err(GmshError::NotBuilt)
    }
}

#[cfg(feature = "gmsh")]
mod backend {
    use super::{GmshError, MeshStats};
    use std::collections::HashMap;
    use std::ffi::CString;
    use std::f64::consts::{FRAC_PI_2, TAU};
    use std::os::raw::{c_char, c_double, c_int, c_void};
    use std::ptr;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    #[allow(non_snake_case)]
    extern "C" {
        fn gmshInitialize(argc: c_int, argv: *mut *mut c_char, readConfigFiles: c_int, run: c_int, ierr: *mut c_int);
        fn gmshFinalize(ierr: *mut c_int);
        fn gmshFree(p: *mut c_void);
        fn gmshOptionSetNumber(name: *const c_char, value: c_double, ierr: *mut c_int);
        fn gmshModelAdd(name: *const c_char, ierr: *mut c_int);
        fn gmshModelGetBoundingBox(dim: c_int, tag: c_int, xmin: *mut c_double, ymin: *mut c_double, zmin: *mut c_double, xmax: *mut c_double, ymax: *mut c_double, zmax: *mut c_double, ierr: *mut c_int);
        fn gmshModelOccAddBox(x: c_double, y: c_double, z: c_double, dx: c_double, dy: c_double, dz: c_double, tag: c_int, ierr: *mut c_int) -> c_int;
        fn gmshModelOccAddSphere(xc: c_double, yc: c_double, zc: c_double, radius: c_double, tag: c_int, a1: c_double, a2: c_double, a3: c_double, ierr: *mut c_int) -> c_int;
        fn gmshModelOccAddCylinder(x: c_double, y: c_double, z: c_double, dx: c_double, dy: c_double, dz: c_double, r: c_double, tag: c_int, angle: c_double, ierr: *mut c_int) -> c_int;
        fn gmshModelOccAddCone(x: c_double, y: c_double, z: c_double, dx: c_double, dy: c_double, dz: c_double, r1: c_double, r2: c_double, tag: c_int, angle: c_double, ierr: *mut c_int) -> c_int;
        #[allow(clippy::too_many_arguments)]
        fn gmshModelOccCut(obj: *const c_int, obj_n: usize, tool: *const c_int, tool_n: usize, out: *mut *mut c_int, out_n: *mut usize, out_map: *mut *mut *mut c_int, out_map_n: *mut *mut usize, out_map_nn: *mut usize, tag: c_int, remove_object: c_int, remove_tool: c_int, ierr: *mut c_int);
        #[allow(clippy::too_many_arguments)]
        fn gmshModelOccFuse(obj: *const c_int, obj_n: usize, tool: *const c_int, tool_n: usize, out: *mut *mut c_int, out_n: *mut usize, out_map: *mut *mut *mut c_int, out_map_n: *mut *mut usize, out_map_nn: *mut usize, tag: c_int, remove_object: c_int, remove_tool: c_int, ierr: *mut c_int);
        #[allow(clippy::too_many_arguments)]
        fn gmshModelOccHealShapes(out: *mut *mut c_int, out_n: *mut usize, dim_tags: *const c_int, dim_tags_n: usize, tolerance: c_double, fix_degenerated: c_int, fix_small_edges: c_int, fix_small_faces: c_int, sew_faces: c_int, make_solids: c_int, ierr: *mut c_int);
        fn gmshModelOccSynchronize(ierr: *mut c_int);
        fn gmshModelMeshGenerate(dim: c_int, ierr: *mut c_int);
        #[allow(clippy::too_many_arguments)]
        fn gmshModelMeshGetNodes(node_tags: *mut *mut usize, node_tags_n: *mut usize, coord: *mut *mut c_double, coord_n: *mut usize, parametric_coord: *mut *mut c_double, parametric_coord_n: *mut usize, dim: c_int, tag: c_int, include_boundary: c_int, return_parametric: c_int, ierr: *mut c_int);
        #[allow(clippy::too_many_arguments)]
        fn gmshModelMeshGetElementsByType(element_type: c_int, element_tags: *mut *mut usize, element_tags_n: *mut usize, node_tags: *mut *mut usize, node_tags_n: *mut usize, tag: c_int, task: usize, num_tasks: usize, ierr: *mut c_int);
    }

    // Gmsh mesh element type ids.
    const ELEM_TRIANGLE: c_int = 2; // 3-node triangle (surface)
    const ELEM_TET: c_int = 4; // 4-node tetrahedron (volume)

    fn lock() -> MutexGuard<'static, ()> {
        static M: OnceLock<Mutex<()>> = OnceLock::new();
        M.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|e| e.into_inner())
    }

    fn check(ierr: c_int, what: &str) -> Result<(), GmshError> {
        if ierr == 0 { Ok(()) } else { Err(GmshError::Failed(format!("{what} (ierr={ierr})"))) }
    }

    unsafe fn set_option(name: &str, value: f64) {
        let mut e = 0;
        let n = CString::new(name).unwrap();
        gmshOptionSetNumber(n.as_ptr(), value, &mut e);
    }

    unsafe fn take_f64(ptr: *mut c_double, n: usize) -> Vec<f64> {
        if ptr.is_null() || n == 0 { return Vec::new(); }
        let v = std::slice::from_raw_parts(ptr, n).to_vec();
        gmshFree(ptr as *mut c_void);
        v
    }
    unsafe fn take_usize(ptr: *mut usize, n: usize) -> Vec<usize> {
        if ptr.is_null() || n == 0 { return Vec::new(); }
        let v = std::slice::from_raw_parts(ptr, n).to_vec();
        gmshFree(ptr as *mut c_void);
        v
    }
    unsafe fn take_int(ptr: *mut c_int, n: usize) -> Vec<c_int> {
        if ptr.is_null() || n == 0 { return Vec::new(); }
        let v = std::slice::from_raw_parts(ptr, n).to_vec();
        gmshFree(ptr as *mut c_void);
        v
    }
    /// Free the per-input mapping (`int***` + `size_t**`) returned by boolean ops.
    unsafe fn free_map(map: *mut *mut c_int, map_n: *mut usize, map_nn: usize) {
        if !map.is_null() {
            for i in 0..map_nn {
                let inner = *map.add(i);
                if !inner.is_null() { gmshFree(inner as *mut c_void); }
            }
            gmshFree(map as *mut c_void);
        }
        if !map_n.is_null() { gmshFree(map_n as *mut c_void); }
    }

    /// Options for [`GmshSession::heal`] (maps to `occ.healShapes`).
    #[derive(Clone, Copy, Debug)]
    pub struct HealOptions {
        pub tolerance: f64,
        pub fix_degenerated: bool,
        pub fix_small_edges: bool,
        pub fix_small_faces: bool,
        pub sew_faces: bool,
        pub make_solids: bool,
    }
    impl Default for HealOptions {
        fn default() -> Self {
            Self { tolerance: 1e-6, fix_degenerated: true, fix_small_edges: true, fix_small_faces: true, sew_faces: true, make_solids: true }
        }
    }

    /// A surface triangulation for the viewport (positions + indices; the renderer
    /// computes normals). Matches the `cad.tessellate_adaptive` result shape.
    #[derive(Debug, Clone)]
    pub struct TriMesh {
        pub positions: Vec<f64>,
        pub indices: Vec<u32>,
        pub triangle_count: usize,
    }

    /// A tetrahedral volume mesh for the solver.
    #[derive(Debug, Clone)]
    pub struct VolumeMesh {
        pub coords: Vec<f64>,
        pub tets: Vec<u32>,
        pub node_count: usize,
        pub tet_count: usize,
    }

    /// An exclusive Gmsh session. Holds a process-global lock + initialized Gmsh
    /// context for its lifetime; `Drop` finalizes Gmsh and releases the lock.
    pub struct GmshSession {
        _guard: MutexGuard<'static, ()>,
    }

    impl GmshSession {
        /// Acquire the global lock and initialize Gmsh (quiet, no GUI).
        pub fn new() -> Result<Self, GmshError> {
            let guard = lock();
            let mut ierr = 0;
            unsafe {
                gmshInitialize(0, ptr::null_mut(), 0, 0, &mut ierr);
            }
            check(ierr, "initialize")?;
            unsafe { set_option("General.Terminal", 0.0) }; // silence console
            Ok(Self { _guard: guard })
        }

        /// Start a fresh, empty model (clears any prior geometry).
        pub fn reset(&self) -> Result<(), GmshError> {
            let mut e = 0;
            let name = CString::new("gfd").unwrap();
            unsafe { gmshModelAdd(name.as_ptr(), &mut e) };
            check(e, "model.add")
        }

        pub fn add_box(&self, x: f64, y: f64, z: f64, dx: f64, dy: f64, dz: f64) -> Result<i32, GmshError> {
            let mut e = 0;
            let t = unsafe { gmshModelOccAddBox(x, y, z, dx, dy, dz, -1, &mut e) };
            check(e, "occ.addBox")?;
            Ok(t)
        }

        pub fn add_sphere(&self, xc: f64, yc: f64, zc: f64, r: f64) -> Result<i32, GmshError> {
            let mut e = 0;
            let t = unsafe { gmshModelOccAddSphere(xc, yc, zc, r, -1, -FRAC_PI_2, FRAC_PI_2, TAU, &mut e) };
            check(e, "occ.addSphere")?;
            Ok(t)
        }

        pub fn add_cylinder(&self, x: f64, y: f64, z: f64, dx: f64, dy: f64, dz: f64, r: f64) -> Result<i32, GmshError> {
            let mut e = 0;
            let t = unsafe { gmshModelOccAddCylinder(x, y, z, dx, dy, dz, r, -1, TAU, &mut e) };
            check(e, "occ.addCylinder")?;
            Ok(t)
        }

        pub fn add_cone(&self, x: f64, y: f64, z: f64, dx: f64, dy: f64, dz: f64, r1: f64, r2: f64) -> Result<i32, GmshError> {
            let mut e = 0;
            let t = unsafe { gmshModelOccAddCone(x, y, z, dx, dy, dz, r1, r2, -1, TAU, &mut e) };
            check(e, "occ.addCone")?;
            Ok(t)
        }

        /// Boolean cut: `objects − tools`. Returns the resulting solid tags.
        pub fn cut(&self, objects: &[i32], tools: &[i32], remove: bool) -> Result<Vec<i32>, GmshError> {
            self.boolean(true, objects, tools, remove)
        }

        /// Boolean fuse (union). Returns the resulting solid tags.
        pub fn fuse(&self, objects: &[i32], tools: &[i32], remove: bool) -> Result<Vec<i32>, GmshError> {
            self.boolean(false, objects, tools, remove)
        }

        fn boolean(&self, is_cut: bool, objects: &[i32], tools: &[i32], remove: bool) -> Result<Vec<i32>, GmshError> {
            let obj: Vec<c_int> = objects.iter().flat_map(|&t| [3, t]).collect();
            let tool: Vec<c_int> = tools.iter().flat_map(|&t| [3, t]).collect();
            let mut out: *mut c_int = ptr::null_mut();
            let mut out_n: usize = 0;
            let mut map: *mut *mut c_int = ptr::null_mut();
            let mut map_n: *mut usize = ptr::null_mut();
            let mut map_nn: usize = 0;
            let mut e = 0;
            let r = remove as c_int;
            unsafe {
                if is_cut {
                    gmshModelOccCut(obj.as_ptr(), obj.len(), tool.as_ptr(), tool.len(), &mut out, &mut out_n, &mut map, &mut map_n, &mut map_nn, -1, r, r, &mut e);
                } else {
                    gmshModelOccFuse(obj.as_ptr(), obj.len(), tool.as_ptr(), tool.len(), &mut out, &mut out_n, &mut map, &mut map_n, &mut map_nn, -1, r, r, &mut e);
                }
            }
            check(e, if is_cut { "occ.cut" } else { "occ.fuse" })?;
            let flat = unsafe { take_int(out, out_n) };
            unsafe { free_map(map, map_n, map_nn) };
            Ok(flat.chunks_exact(2).filter(|c| c[0] == 3).map(|c| c[1]).collect())
        }

        /// Heal the given solids (empty = heal all). Returns the healed solid tags.
        pub fn heal(&self, tags: &[i32], opts: HealOptions) -> Result<Vec<i32>, GmshError> {
            let dt: Vec<c_int> = tags.iter().flat_map(|&t| [3, t]).collect();
            let mut out: *mut c_int = ptr::null_mut();
            let mut out_n: usize = 0;
            let mut e = 0;
            unsafe {
                gmshModelOccHealShapes(
                    &mut out, &mut out_n,
                    dt.as_ptr(), dt.len(),
                    opts.tolerance,
                    opts.fix_degenerated as c_int, opts.fix_small_edges as c_int, opts.fix_small_faces as c_int,
                    opts.sew_faces as c_int, opts.make_solids as c_int,
                    &mut e,
                );
            }
            check(e, "occ.healShapes")?;
            let flat = unsafe { take_int(out, out_n) };
            Ok(flat.chunks_exact(2).filter(|c| c[0] == 3).map(|c| c[1]).collect())
        }

        /// Push pending OCC geometry into the Gmsh model (required before meshing / bbox).
        pub fn synchronize(&self) -> Result<(), GmshError> {
            let mut e = 0;
            unsafe { gmshModelOccSynchronize(&mut e) };
            check(e, "occ.synchronize")
        }

        /// Axis-aligned bounding box `[xmin,ymin,zmin,xmax,ymax,zmax]` of a synchronized solid.
        pub fn bounding_box(&self, tag: i32) -> Result<[f64; 6], GmshError> {
            let mut b = [0.0f64; 6];
            let mut e = 0;
            unsafe {
                gmshModelGetBoundingBox(3, tag, &mut b[0], &mut b[1], &mut b[2], &mut b[3], &mut b[4], &mut b[5], &mut e);
            }
            check(e, "getBoundingBox")?;
            Ok(b)
        }

        /// Axis-aligned bounding box of the whole synchronized model (all entities).
        pub fn model_bbox(&self) -> Result<[f64; 6], GmshError> {
            let mut b = [0.0f64; 6];
            let mut e = 0;
            unsafe {
                gmshModelGetBoundingBox(-1, -1, &mut b[0], &mut b[1], &mut b[2], &mut b[3], &mut b[4], &mut b[5], &mut e);
            }
            check(e, "model getBoundingBox")?;
            Ok(b)
        }

        /// Surface tessellation for the viewport (meshes 2D at `max_size`).
        pub fn tessellate(&self, max_size: f64) -> Result<TriMesh, GmshError> {
            self.synchronize()?;
            unsafe { set_option("Mesh.MeshSizeMax", max_size) };
            self.generate(2)?;
            let (coords, conn) = self.build_mesh(ELEM_TRIANGLE)?;
            let triangle_count = conn.len() / 3;
            Ok(TriMesh { positions: coords, indices: conn, triangle_count })
        }

        /// Tetrahedral volume mesh for the solver (meshes 3D at `max_size`).
        pub fn volume_mesh(&self, max_size: f64) -> Result<VolumeMesh, GmshError> {
            self.synchronize()?;
            unsafe { set_option("Mesh.MeshSizeMax", max_size) };
            self.generate(3)?;
            let (coords, conn) = self.build_mesh(ELEM_TET)?;
            let node_count = coords.len() / 3;
            let tet_count = conn.len() / 4;
            Ok(VolumeMesh { coords, tets: conn, node_count, tet_count })
        }

        fn generate(&self, dim: c_int) -> Result<(), GmshError> {
            let mut e = 0;
            unsafe { gmshModelMeshGenerate(dim, &mut e) };
            check(e, "mesh.generate")
        }

        /// Read all nodes: `(gmsh node tags, flat xyz coords)`.
        fn read_nodes(&self) -> Result<(Vec<usize>, Vec<f64>), GmshError> {
            let mut nt: *mut usize = ptr::null_mut();
            let mut nt_n: usize = 0;
            let mut c: *mut c_double = ptr::null_mut();
            let mut c_n: usize = 0;
            let mut p: *mut c_double = ptr::null_mut();
            let mut p_n: usize = 0;
            let mut e = 0;
            unsafe {
                gmshModelMeshGetNodes(&mut nt, &mut nt_n, &mut c, &mut c_n, &mut p, &mut p_n, -1, -1, 1, 0, &mut e);
            }
            check(e, "mesh.getNodes")?;
            let tags = unsafe { take_usize(nt, nt_n) };
            let coords = unsafe { take_f64(c, c_n) };
            unsafe { if !p.is_null() { gmshFree(p as *mut c_void); } }
            Ok((tags, coords))
        }

        /// Connectivity (gmsh node tags) of all elements of the given type.
        fn elements_of_type(&self, et: c_int) -> Result<Vec<usize>, GmshError> {
            let mut etags: *mut usize = ptr::null_mut();
            let mut etags_n: usize = 0;
            let mut ntags: *mut usize = ptr::null_mut();
            let mut ntags_n: usize = 0;
            let mut e = 0;
            unsafe {
                gmshModelMeshGetElementsByType(et, &mut etags, &mut etags_n, &mut ntags, &mut ntags_n, -1, 0, 1, &mut e);
            }
            check(e, "mesh.getElementsByType")?;
            let node_tags = unsafe { take_usize(ntags, ntags_n) };
            unsafe { if !etags.is_null() { gmshFree(etags as *mut c_void); } }
            Ok(node_tags)
        }

        /// Build `(coords, 0-based connectivity)` for the given element type.
        fn build_mesh(&self, element_type: c_int) -> Result<(Vec<f64>, Vec<u32>), GmshError> {
            let (tags, coords) = self.read_nodes()?;
            let mut index: HashMap<usize, u32> = HashMap::with_capacity(tags.len());
            for (i, &t) in tags.iter().enumerate() {
                index.insert(t, i as u32);
            }
            let conn_tags = self.elements_of_type(element_type)?;
            let mut conn = Vec::with_capacity(conn_tags.len());
            for &t in &conn_tags {
                conn.push(*index.get(&t).ok_or_else(|| GmshError::Failed("element references unknown node tag".into()))?);
            }
            Ok((coords, conn))
        }
    }

    impl Drop for GmshSession {
        fn drop(&mut self) {
            let mut e = 0;
            unsafe { gmshFinalize(&mut e) };
        }
    }

    /// Convenience: mesh a box from scratch (proves the FFI pipeline).
    pub fn mesh_box(dx: f64, dy: f64, dz: f64, mesh_size: f64) -> Result<MeshStats, GmshError> {
        let s = GmshSession::new()?;
        s.add_box(0.0, 0.0, 0.0, dx, dy, dz)?;
        let vm = s.volume_mesh(mesh_size)?;
        Ok(MeshStats { nodes: vm.node_count, coord_len: vm.coords.len() })
    }
}

pub use backend::mesh_box;
#[cfg(feature = "gmsh")]
pub use backend::{GmshSession, HealOptions, TriMesh, VolumeMesh};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_matches_feature() {
        assert_eq!(is_available(), cfg!(feature = "gmsh"));
    }

    #[test]
    fn not_built_error_renders() {
        assert!(GmshError::NotBuilt.to_string().contains("gmsh"));
    }

    #[cfg(feature = "gmsh")]
    #[test]
    fn meshes_a_box() {
        let stats = mesh_box(2.0, 1.0, 1.0, 0.5).expect("gmsh meshes a box");
        assert!(stats.nodes >= 8, "got {} nodes", stats.nodes);
        assert_eq!(stats.coord_len, stats.nodes * 3);
    }

    #[cfg(feature = "gmsh")]
    #[test]
    fn enclosure_cut_heal_volume_mesh() {
        // The exact workflow the AI must drive: solid → enclosure → fluid (cut) → heal → volume mesh.
        let s = GmshSession::new().expect("session");
        let part = s.add_box(0.0, 0.0, 0.0, 1.0, 1.0, 1.0).expect("part");
        let enclosure = s.add_box(-1.0, -1.0, -1.0, 3.0, 3.0, 3.0).expect("enclosure");
        let fluid = s.cut(&[enclosure], &[part], true).expect("enclosure - part");
        assert!(!fluid.is_empty(), "cut should yield a fluid solid");
        let healed = s.heal(&fluid, HealOptions::default()).expect("heal");
        assert!(!healed.is_empty(), "heal should keep the fluid solid");
        let vm = s.volume_mesh(0.5).expect("volume mesh");
        assert!(vm.tet_count > 0 && vm.node_count > 0, "fluid mesh: {} nodes {} tets", vm.node_count, vm.tet_count);
    }

    #[cfg(feature = "gmsh")]
    #[test]
    fn tessellates_a_sphere() {
        let s = GmshSession::new().expect("session");
        let sph = s.add_sphere(0.0, 0.0, 0.0, 1.0).expect("sphere");
        s.synchronize().expect("sync");
        let bb = s.bounding_box(sph).expect("bbox");
        assert!((bb[3] - bb[0] - 2.0).abs() < 1e-3, "sphere x-extent ~2, got {}", bb[3] - bb[0]);
        let tri = s.tessellate(0.3).expect("tessellate");
        assert_eq!(tri.indices.len(), tri.triangle_count * 3, "3 indices per triangle");
        let max_idx = tri.indices.iter().copied().max().unwrap_or(0) as usize;
        assert!(tri.positions.len() >= (max_idx + 1) * 3, "every index must map to a position");
        assert!(tri.triangle_count > 50, "sphere should tessellate to many tris, got {}", tri.triangle_count);
    }
}

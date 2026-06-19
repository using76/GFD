//! 2D Shallow-Water Equations (Saint-Venant) — the flood-simulation main engine
//! (see `docs/FLOOD_PLAN.md` §4). Conserved state `U = [h, hu, hv]` over a
//! Cartesian cell grid with a bed elevation field `z_b`.
//!
//! This module is the Phase-0 scaffold: state, parameters, wetting/drying
//! velocity recovery, and the SWE CFL timestep. The full Godunov/HLLC solver
//! with Audusse well-balancing, positivity, and Manning friction lands in
//! Phase 6 (`hllc.rs`, `well_balanced.rs`, `wetting_drying.rs`, `friction.rs`,
//! `muscl.rs`, `solver.rs`).
//!
//! SWE is a 2D hyperbolic conservation law isomorphic to the 2D compressible
//! Euler equations (pressure ½g h², celerity √(gh)), so the existing HLLC +
//! CFL machinery is the implementation template — not a blank slate.

pub mod hllc;
pub mod solver;

pub use hllc::hllc_normal;
pub use solver::{SwBc, SwBoundaries, SwSolver};

/// A uniform Cartesian grid for the SWE solver (built from a DEM in Phase 4).
#[derive(Debug, Clone)]
pub struct SwGrid {
    pub ncols: usize,
    pub nrows: usize,
    pub dx: f64,
    pub dy: f64,
}

impl SwGrid {
    pub fn new(ncols: usize, nrows: usize, dx: f64, dy: f64) -> Self {
        Self { ncols, nrows, dx, dy }
    }
    #[inline]
    pub fn n_cells(&self) -> usize {
        self.ncols * self.nrows
    }
    #[inline]
    pub fn idx(&self, col: usize, row: usize) -> usize {
        row * self.ncols + col
    }
}

/// Conserved SWE state per cell: water depth `h`, and momenta `hu`, `hv`.
/// `z_b` is the (static) bed elevation; free-surface = `z_b + h`.
#[derive(Debug, Clone)]
pub struct SwState {
    pub h: Vec<f64>,
    pub hu: Vec<f64>,
    pub hv: Vec<f64>,
    pub z_b: Vec<f64>,
}

impl SwState {
    /// All-dry state over a bed field.
    pub fn dry(z_b: Vec<f64>) -> Self {
        let n = z_b.len();
        Self { h: vec![0.0; n], hu: vec![0.0; n], hv: vec![0.0; n], z_b }
    }

    /// A still water body filled to a constant free-surface level (lake at rest):
    /// `h = max(0, level − z_b)`, zero momentum. The canonical well-balanced test.
    pub fn lake_at_rest(z_b: Vec<f64>, level: f64) -> Self {
        let h: Vec<f64> = z_b.iter().map(|&zb| (level - zb).max(0.0)).collect();
        let n = z_b.len();
        Self { h, hu: vec![0.0; n], hv: vec![0.0; n], z_b }
    }

    pub fn n_cells(&self) -> usize {
        self.h.len()
    }
}

/// Physical + numerical parameters.
#[derive(Debug, Clone, Copy)]
pub struct SwParams {
    /// Gravitational acceleration [m/s²].
    pub gravity: f64,
    /// Manning roughness coefficient n [s·m^(−1/3)].
    pub manning_n: f64,
    /// Wet/dry threshold [m]; cells with h ≤ h_dry are treated as dry (u = v = 0).
    pub h_dry: f64,
    /// Courant number for the adaptive timestep (≤ 1 for the 1st-order scheme).
    pub cfl: f64,
}

impl Default for SwParams {
    fn default() -> Self {
        Self { gravity: 9.81, manning_n: 0.03, h_dry: 1.0e-4, cfl: 0.45 }
    }
}

/// Depth-averaged velocity `(u, v)` of cell `i`, with wetting/drying: a dry or
/// near-dry cell (h ≤ h_dry) has zero velocity, preventing the `hu/h` blow-up
/// that destabilizes shallow fronts.
#[inline]
pub fn velocity(state: &SwState, i: usize, params: &SwParams) -> (f64, f64) {
    let h = state.h[i];
    if h <= params.h_dry {
        (0.0, 0.0)
    } else {
        (state.hu[i] / h, state.hv[i] / h)
    }
}

/// Maximum signal speed over all wet cells: max(|u| + √(g h), |v| + √(g h)).
pub fn max_wave_speed(state: &SwState, params: &SwParams) -> f64 {
    let mut s = 0.0_f64;
    for i in 0..state.n_cells() {
        let h = state.h[i];
        if h <= params.h_dry {
            continue;
        }
        let (u, v) = velocity(state, i, params);
        let c = (params.gravity * h).sqrt();
        s = s.max(u.abs() + c).max(v.abs() + c);
    }
    s
}

/// Adaptive CFL timestep for the SWE system: `dt = cfl · min(dx,dy) / smax`,
/// where `smax = max(|u| + √(gh))`. Returns `None` when everything is dry/still
/// (no finite signal speed) — the caller picks a default/forced dt.
pub fn cfl_timestep(grid: &SwGrid, state: &SwState, params: &SwParams) -> Option<f64> {
    let smax = max_wave_speed(state, params);
    if smax <= 0.0 || !smax.is_finite() {
        return None;
    }
    Some(params.cfl * grid.dx.min(grid.dy) / smax)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lake_at_rest_fills_below_level_only() {
        // Bed: 0, 1, 3 ; level 2 → depths 2, 1, 0.
        let s = SwState::lake_at_rest(vec![0.0, 1.0, 3.0], 2.0);
        assert_eq!(s.h, vec![2.0, 1.0, 0.0]);
        assert!(s.hu.iter().all(|&m| m == 0.0));
    }

    #[test]
    fn dry_cell_velocity_is_zero() {
        let p = SwParams::default();
        let mut s = SwState::dry(vec![0.0, 0.0]);
        s.h = vec![1.0e-6, 2.0];
        s.hu = vec![5.0, 4.0]; // even with momentum, the near-dry cell reads zero
        assert_eq!(velocity(&s, 0, &p), (0.0, 0.0));
        assert_eq!(velocity(&s, 1, &p), (2.0, 0.0));
    }

    #[test]
    fn cfl_timestep_matches_celerity() {
        let p = SwParams::default();
        let grid = SwGrid::new(2, 1, 10.0, 10.0);
        // Still lake, depth 1 m → smax = √(g·1); dt = cfl·dx/smax.
        let s = SwState::lake_at_rest(vec![-1.0, -1.0], 0.0);
        let c = (p.gravity * 1.0).sqrt();
        let dt = cfl_timestep(&grid, &s, &p).unwrap();
        assert!((dt - p.cfl * 10.0 / c).abs() < 1e-9, "dt={dt}");
    }

    #[test]
    fn cfl_none_when_all_dry() {
        let p = SwParams::default();
        let grid = SwGrid::new(2, 1, 1.0, 1.0);
        let s = SwState::dry(vec![0.0, 0.0]);
        assert!(cfl_timestep(&grid, &s, &p).is_none());
    }
}

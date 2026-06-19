//! Well-balanced finite-volume SWE solver (1st-order Godunov in space, SSP-RK2
//! in time) on a uniform Cartesian grid.
//!
//! - Flux: HLLC ([`super::hllc::hllc_normal`]).
//! - Well-balancing: Audusse (2004) hydrostatic reconstruction — exact lake-at-
//!   rest C-property (still water over arbitrary bed stays still to machine ε).
//! - Wetting/drying + positivity: reconstructed depths are clamped ≥ 0; cells
//!   below `h_dry` are reset to zero velocity each stage.
//! - Friction: Manning, semi-implicit (operator-split after the hyperbolic update).
//! - Boundaries: wall (reflect normal velocity), transmissive (zero-gradient),
//!   or a fixed free-surface depth.

use super::hllc::hllc_normal;
use super::{velocity, SwGrid, SwParams, SwState};

/// Per-side boundary condition.
#[derive(Debug, Clone, Copy)]
pub enum SwBc {
    /// Reflective solid wall (no normal flow) — buildings, levees, closed edges.
    Wall,
    /// Zero-gradient open boundary (waves leave cleanly).
    Transmissive,
    /// Imposed water depth at the boundary (e.g. downstream stage).
    FixedDepth(f64),
}

/// Domain-edge boundary conditions.
#[derive(Debug, Clone, Copy)]
pub struct SwBoundaries {
    pub xmin: SwBc,
    pub xmax: SwBc,
    pub ymin: SwBc,
    pub ymax: SwBc,
}

impl Default for SwBoundaries {
    fn default() -> Self {
        Self { xmin: SwBc::Wall, xmax: SwBc::Wall, ymin: SwBc::Wall, ymax: SwBc::Wall }
    }
}

pub struct SwSolver {
    pub grid: SwGrid,
    pub params: SwParams,
    pub bc: SwBoundaries,
}

/// A cell's primitive snapshot used in flux evaluation.
#[derive(Clone, Copy)]
struct Cell {
    h: f64,
    u: f64,
    v: f64,
    zb: f64,
}

impl SwSolver {
    pub fn new(grid: SwGrid, params: SwParams, bc: SwBoundaries) -> Self {
        Self { grid, params, bc }
    }

    #[inline]
    fn cell(&self, s: &SwState, i: usize) -> Cell {
        let (u, v) = velocity(s, i, &self.params);
        Cell { h: s.h[i].max(0.0), u, v, zb: s.z_b[i] }
    }

    /// Ghost cell across a boundary face, given the interior cell and the BC.
    /// `normal_is_x` selects which velocity component is the face normal.
    fn ghost(&self, interior: Cell, bc: SwBc, normal_is_x: bool) -> Cell {
        match bc {
            SwBc::Wall => {
                let mut g = interior;
                if normal_is_x { g.u = -interior.u; } else { g.v = -interior.v; }
                g
            }
            SwBc::Transmissive => interior,
            SwBc::FixedDepth(level) => {
                let mut g = interior;
                g.h = (level - interior.zb).max(0.0);
                g
            }
        }
    }

    /// One Audusse-reconstructed HLLC interface flux between a left and right
    /// cell along a normal axis. Returns `(flux, src_l, src_r)` where `flux` is
    /// `[F_h, F_qn, F_qt]` and `src_l`/`src_r` are the hydrostatic momentum-source
    /// corrections added to the left/right cell's normal-momentum balance.
    fn interface(&self, l: Cell, r: Cell, normal_is_x: bool) -> ([f64; 3], f64, f64) {
        let g = self.params.gravity;
        let zmax = l.zb.max(r.zb);
        let hl = (l.h + l.zb - zmax).max(0.0); // η_l − zmax
        let hr = (r.h + r.zb - zmax).max(0.0);
        let (unl, utl, unr, utr) = if normal_is_x {
            (l.u, l.v, r.u, r.v)
        } else {
            (l.v, l.u, r.v, r.u)
        };
        let f = hllc_normal(hl, unl, utl, hr, unr, utr, g);
        // Audusse hydrostatic source corrections (give exact C-property).
        let src_l = 0.5 * g * (l.h * l.h - hl * hl);
        let src_r = 0.5 * g * (r.h * r.h - hr * hr);
        (f, src_l, src_r)
    }

    /// Spatial residual dU/dt for every cell (hyperbolic terms only).
    fn residual(&self, s: &SwState, rh: &mut [f64], rhu: &mut [f64], rhv: &mut [f64]) {
        let (nc, nr) = (self.grid.ncols, self.grid.nrows);
        let (dx, dy) = (self.grid.dx, self.grid.dy);
        rh.iter_mut().for_each(|x| *x = 0.0);
        rhu.iter_mut().for_each(|x| *x = 0.0);
        rhv.iter_mut().for_each(|x| *x = 0.0);

        // ---- x-direction faces (normal = x; normal momentum = hu) ----
        for r in 0..nr {
            for c in 0..=nc {
                // interface between cell (c-1,r) [left] and (c,r) [right]
                let li = if c > 0 { Some(self.grid.idx(c - 1, r)) } else { None };
                let ri = if c < nc { Some(self.grid.idx(c, r)) } else { None };
                let (l, r_cell) = match (li, ri) {
                    (Some(li), Some(ri)) => (self.cell(s, li), self.cell(s, ri)),
                    (None, Some(ri)) => { let ic = self.cell(s, ri); (self.ghost(ic, self.bc.xmin, true), ic) }
                    (Some(li), None) => { let ic = self.cell(s, li); (ic, self.ghost(ic, self.bc.xmax, true)) }
                    (None, None) => continue,
                };
                let (f, src_l, src_r) = self.interface(l, r_cell, true);
                if let Some(li) = li {
                    rh[li] -= f[0] / dx;
                    rhu[li] -= (f[1] + src_l) / dx;
                    rhv[li] -= f[2] / dx;
                }
                if let Some(ri) = ri {
                    rh[ri] += f[0] / dx;
                    rhu[ri] += (f[1] + src_r) / dx;
                    rhv[ri] += f[2] / dx;
                }
            }
        }

        // ---- y-direction faces (normal = y; normal momentum = hv) ----
        for c in 0..nc {
            for r in 0..=nr {
                let li = if r > 0 { Some(self.grid.idx(c, r - 1)) } else { None };
                let ri = if r < nr { Some(self.grid.idx(c, r)) } else { None };
                let (l, r_cell) = match (li, ri) {
                    (Some(li), Some(ri)) => (self.cell(s, li), self.cell(s, ri)),
                    (None, Some(ri)) => { let ic = self.cell(s, ri); (self.ghost(ic, self.bc.ymin, false), ic) }
                    (Some(li), None) => { let ic = self.cell(s, li); (ic, self.ghost(ic, self.bc.ymax, false)) }
                    (None, None) => continue,
                };
                let (f, src_l, src_r) = self.interface(l, r_cell, false);
                // For y-faces the HLLC normal momentum is hv; transverse is hu.
                if let Some(li) = li {
                    rh[li] -= f[0] / dy;
                    rhv[li] -= (f[1] + src_l) / dy;
                    rhu[li] -= f[2] / dy;
                }
                if let Some(ri) = ri {
                    rh[ri] += f[0] / dy;
                    rhv[ri] += (f[1] + src_r) / dy;
                    rhu[ri] += f[2] / dy;
                }
            }
        }
    }

    /// Forward-Euler stage: `out = in + dt·R(in)`, then positivity clamp.
    fn euler_stage(&self, s: &SwState, dt: f64, scratch: &mut Scratch, out: &mut SwState) {
        self.residual(s, &mut scratch.rh, &mut scratch.rhu, &mut scratch.rhv);
        for i in 0..s.n_cells() {
            out.h[i] = s.h[i] + dt * scratch.rh[i];
            out.hu[i] = s.hu[i] + dt * scratch.rhu[i];
            out.hv[i] = s.hv[i] + dt * scratch.rhv[i];
        }
        self.positivity(out);
    }

    /// Clamp depths ≥ 0 and zero momentum in dry cells.
    fn positivity(&self, s: &mut SwState) {
        for i in 0..s.n_cells() {
            if s.h[i] <= self.params.h_dry {
                s.h[i] = s.h[i].max(0.0);
                s.hu[i] = 0.0;
                s.hv[i] = 0.0;
            }
        }
    }

    /// Manning friction, semi-implicit: d(hu)/dt = −k·hu with
    /// k = g n² |U| / h^{4/3}; solved as hu ← hu / (1 + dt·k).
    fn friction(&self, s: &mut SwState, dt: f64) {
        let n = self.params.manning_n;
        if n <= 0.0 {
            return;
        }
        let g = self.params.gravity;
        for i in 0..s.n_cells() {
            let h = s.h[i];
            if h <= self.params.h_dry {
                continue;
            }
            let (u, v) = velocity(s, i, &self.params);
            let speed = (u * u + v * v).sqrt();
            let k = g * n * n * speed / h.powf(4.0 / 3.0);
            let f = 1.0 / (1.0 + dt * k);
            s.hu[i] *= f;
            s.hv[i] *= f;
        }
    }

    /// Advance one step by `dt` with SSP-RK2 (Heun) + operator-split friction.
    pub fn step(&self, s: &mut SwState, dt: f64) {
        let n = s.n_cells();
        let mut scratch = Scratch::new(n);
        let mut u1 = s.clone();
        // Stage 1: U1 = U + dt R(U)
        self.euler_stage(s, dt, &mut scratch, &mut u1);
        // Stage 2: U2 = U1 + dt R(U1); then U^{n+1} = ½(U + U2)
        let mut u2 = u1.clone();
        self.euler_stage(&u1, dt, &mut scratch, &mut u2);
        for i in 0..n {
            s.h[i] = 0.5 * (s.h[i] + u2.h[i]);
            s.hu[i] = 0.5 * (s.hu[i] + u2.hu[i]);
            s.hv[i] = 0.5 * (s.hv[i] + u2.hv[i]);
        }
        self.positivity(s);
        self.friction(s, dt);
    }

    /// Run to `t_end` using the adaptive CFL timestep. Returns the steps taken.
    pub fn advance(&self, s: &mut SwState, t_end: f64, max_steps: usize) -> usize {
        let mut t = 0.0;
        let mut steps = 0;
        while t < t_end && steps < max_steps {
            let dt = super::cfl_timestep(&self.grid, s, &self.params)
                .unwrap_or(t_end - t)
                .min(t_end - t);
            if dt <= 0.0 {
                break;
            }
            self.step(s, dt);
            t += dt;
            steps += 1;
        }
        steps
    }

    /// Total water volume Σ h·dx·dy (mass-conservation diagnostic).
    pub fn total_volume(&self, s: &SwState) -> f64 {
        let cell_area = self.grid.dx * self.grid.dy;
        s.h.iter().map(|&h| h.max(0.0)).sum::<f64>() * cell_area
    }
}

struct Scratch {
    rh: Vec<f64>,
    rhu: Vec<f64>,
    rhv: Vec<f64>,
}
impl Scratch {
    fn new(n: usize) -> Self {
        Self { rh: vec![0.0; n], rhu: vec![0.0; n], rhv: vec![0.0; n] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(ncols: usize, nrows: usize) -> Vec<f64> {
        vec![0.0; ncols * nrows]
    }

    #[test]
    fn lake_at_rest_c_property() {
        // Still water over a BUMPY bed must stay still to machine precision.
        let (nc, nr) = (40, 1);
        let mut zb = flat(nc, nr);
        for c in 0..nc {
            zb[c] = 0.3 * (c as f64 * 0.4).sin().abs(); // irregular bumps, all < 0.5
        }
        let s0 = SwState::lake_at_rest(zb, 0.6); // level above all bumps
        let grid = SwGrid::new(nc, nr, 1.0, 1.0);
        let solver = SwSolver::new(grid, SwParams { manning_n: 0.0, ..Default::default() }, SwBoundaries::default());
        let mut s = s0.clone();
        for _ in 0..50 {
            solver.step(&mut s, 0.05);
        }
        // Free surface must remain flat at 0.6 and velocities ~0.
        for i in 0..s.n_cells() {
            assert!((s.h[i] + s.z_b[i] - 0.6).abs() < 1e-10, "eta drift at {i}: {}", s.h[i] + s.z_b[i]);
            assert!(s.hu[i].abs() < 1e-9, "spurious momentum at {i}: {}", s.hu[i]);
        }
    }

    #[test]
    fn dam_break_ritter_front_and_mass() {
        // 1D dry-bed dam break, frictionless flat bed. Ritter analytic: at the dam
        // the depth is 4/9·hL and the wet front travels at 2√(g·hL).
        let (nc, nr) = (400, 1);
        let dx = 1.0;
        let g = 9.81;
        let hl = 10.0;
        let dam = nc / 2;
        let mut s = SwState::dry(flat(nc, nr));
        for c in 0..dam {
            s.h[c] = hl;
        }
        let grid = SwGrid::new(nc, nr, dx, 1.0);
        let solver = SwSolver::new(
            grid,
            SwParams { gravity: g, manning_n: 0.0, ..Default::default() },
            SwBoundaries { xmin: SwBc::Transmissive, xmax: SwBc::Transmissive, ..Default::default() },
        );
        let v0 = solver.total_volume(&s);
        let t = 4.0;
        solver.advance(&mut s, t, 100000);

        // Positivity + finiteness.
        assert!(s.h.iter().all(|&h| h >= 0.0 && h.is_finite()));
        // Mass conserved (no flow reached the open ends in 4 s; front speed ≈ 19.8 m/s
        // → ~79 m < 200 m to the boundary).
        let v1 = solver.total_volume(&s);
        assert!((v1 - v0).abs() / v0 < 1e-3, "mass drift {}", (v1 - v0).abs() / v0);
        // Depth at the dam ≈ 4/9·hL (1st-order diffusion → 12% tolerance).
        let h_dam = s.h[dam];
        let expect = 4.0 / 9.0 * hl;
        assert!((h_dam - expect).abs() / expect < 0.12, "h_dam={h_dam} expect={expect}");
        // Wet front has advanced downstream of the dam but not past the analytic tip.
        let tip = dam as f64 * dx + 2.0 * (g * hl).sqrt() * t; // analytic tip x
        let mut front = dam;
        for c in dam..nc {
            if s.h[c] > 0.01 {
                front = c;
            }
        }
        let front_x = front as f64 * dx;
        assert!(front_x > dam as f64 * dx, "front did not advance");
        assert!(front_x <= tip + 5.0 * dx, "front {front_x} overran analytic tip {tip}");
    }

    #[test]
    fn drained_basin_stays_positive() {
        // A puddle in a bowl with transmissive walls should drain without going
        // negative or NaN (wetting/drying robustness).
        let (nc, nr) = (30, 1);
        let mut s = SwState::dry(flat(nc, nr));
        for c in 12..18 {
            s.h[c] = 1.0;
        }
        let grid = SwGrid::new(nc, nr, 1.0, 1.0);
        let solver = SwSolver::new(
            grid,
            SwParams { manning_n: 0.02, ..Default::default() },
            SwBoundaries { xmin: SwBc::Transmissive, xmax: SwBc::Transmissive, ..Default::default() },
        );
        solver.advance(&mut s, 10.0, 100000);
        assert!(s.h.iter().all(|&h| h >= 0.0 && h.is_finite()));
    }
}

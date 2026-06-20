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
    /// Optional per-cell Manning n override (Phase 5 Roughness burn-in). When set,
    /// `friction()` uses `manning_field[i]` instead of the scalar `params.manning_n`.
    pub manning_field: Option<Vec<f64>>,
    /// Optional per-cell solid mask (Phase 5 Hole burn-in): masked cells act as
    /// internal reflective walls and are kept permanently dry.
    pub solid_mask: Option<Vec<bool>>,
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
        Self { grid, params, bc, manning_field: None, solid_mask: None }
    }

    /// True if cell `i` is a solid (internal-wall) cell.
    #[inline]
    fn masked(&self, i: usize) -> bool {
        self.solid_mask.as_ref().is_some_and(|m| m.get(i).copied().unwrap_or(false))
    }

    /// Mark cell `i` as a solid internal wall (Hole burn-in), allocating the mask
    /// lazily.
    pub fn set_solid(&mut self, i: usize) {
        let n = self.grid.ncols * self.grid.nrows;
        self.solid_mask.get_or_insert_with(|| vec![false; n])[i] = true;
    }

    /// Set the Manning n of cell `i` (Roughness burn-in), allocating the field
    /// lazily (seeded from the scalar `params.manning_n`).
    pub fn set_manning(&mut self, i: usize, n: f64) {
        let nc = self.grid.ncols * self.grid.nrows;
        let base = self.params.manning_n;
        self.manning_field.get_or_insert_with(|| vec![base; nc])[i] = n;
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

    /// Spatial residual dU/dt for every cell (hyperbolic terms only). For
    /// `order >= 2` it uses MUSCL reconstruction of the free surface η = h + z_b
    /// (and u, v, bed z) with minmod limiting — well-balanced (the centered bed
    /// source exactly cancels the reconstructed-face pressure imbalance at rest).
    fn residual(&self, s: &SwState, rh: &mut [f64], rhu: &mut [f64], rhv: &mut [f64]) {
        let (nc, nr) = (self.grid.ncols, self.grid.nrows);
        let (dx, dy) = (self.grid.dx, self.grid.dy);
        let n = s.n_cells();
        rh.iter_mut().for_each(|x| *x = 0.0);
        rhu.iter_mut().for_each(|x| *x = 0.0);
        rhv.iter_mut().for_each(|x| *x = 0.0);

        let muscl = self.params.order >= 2;

        // Cell primitives + free surface.
        let mut up = vec![0.0; n];
        let mut vp = vec![0.0; n];
        let mut eta = vec![0.0; n];
        for i in 0..n {
            let (u, v) = velocity(s, i, &self.params);
            up[i] = u;
            vp[i] = v;
            eta[i] = s.h[i].max(0.0) + s.z_b[i];
        }
        // minmod-limited slopes (per cell, per direction). Zero unless MUSCL, and
        // zero on boundary cells (one-sided → drop to 1st order there).
        let (mut sxe, mut sxu, mut sxv, mut sxz) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        let (mut sye, mut syu, mut syv, mut syz) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        if muscl {
            for r in 0..nr {
                for c in 1..nc.saturating_sub(1) {
                    let i = self.grid.idx(c, r);
                    let (l, rr) = (self.grid.idx(c - 1, r), self.grid.idx(c + 1, r));
                    sxe[i] = minmod(eta[i] - eta[l], eta[rr] - eta[i]);
                    sxu[i] = minmod(up[i] - up[l], up[rr] - up[i]);
                    sxv[i] = minmod(vp[i] - vp[l], vp[rr] - vp[i]);
                    sxz[i] = minmod(s.z_b[i] - s.z_b[l], s.z_b[rr] - s.z_b[i]);
                }
            }
            for c in 0..nc {
                for r in 1..nr.saturating_sub(1) {
                    let i = self.grid.idx(c, r);
                    let (b, t) = (self.grid.idx(c, r - 1), self.grid.idx(c, r + 1));
                    sye[i] = minmod(eta[i] - eta[b], eta[t] - eta[i]);
                    syu[i] = minmod(up[i] - up[b], up[t] - up[i]);
                    syv[i] = minmod(vp[i] - vp[b], vp[t] - vp[i]);
                    syz[i] = minmod(s.z_b[i] - s.z_b[b], s.z_b[t] - s.z_b[i]);
                }
            }
        }
        // Reconstructed face Cell of cell `i`, `side` = +1 (high face) / −1 (low).
        let rx = |i: usize, side: f64| -> Cell {
            let z = s.z_b[i] + side * 0.5 * sxz[i];
            Cell { h: (eta[i] + side * 0.5 * sxe[i] - z).max(0.0), u: up[i] + side * 0.5 * sxu[i], v: vp[i] + side * 0.5 * sxv[i], zb: z }
        };
        let ry = |i: usize, side: f64| -> Cell {
            let z = s.z_b[i] + side * 0.5 * syz[i];
            Cell { h: (eta[i] + side * 0.5 * sye[i] - z).max(0.0), u: up[i] + side * 0.5 * syu[i], v: vp[i] + side * 0.5 * syv[i], zb: z }
        };

        // ---- x-direction faces (normal = x; normal momentum = hu) ----
        // A solid (masked) cell acts as a reflective internal wall: the fluid
        // neighbour sees a wall ghost and the masked cell receives no flux.
        for r in 0..nr {
            for c in 0..=nc {
                let li = if c > 0 { Some(self.grid.idx(c - 1, r)) } else { None };
                let ri = if c < nc { Some(self.grid.idx(c, r)) } else { None };
                let ml = li.is_some_and(|i| self.masked(i));
                let mr = ri.is_some_and(|i| self.masked(i));
                let (l, r_cell, apply_l, apply_r) = match (li, ri) {
                    (Some(li), Some(ri)) => {
                        if ml && mr { continue; }
                        if mr { let ic = rx(li, 1.0); (ic, self.ghost(ic, SwBc::Wall, true), true, false) }
                        else if ml { let ic = rx(ri, -1.0); (self.ghost(ic, SwBc::Wall, true), ic, false, true) }
                        else { (rx(li, 1.0), rx(ri, -1.0), true, true) }
                    }
                    (None, Some(ri)) => { if mr { continue; } let ic = rx(ri, -1.0); (self.ghost(ic, self.bc.xmin, true), ic, false, true) }
                    (Some(li), None) => { if ml { continue; } let ic = rx(li, 1.0); (ic, self.ghost(ic, self.bc.xmax, true), true, false) }
                    (None, None) => continue,
                };
                let (f, src_l, src_r) = self.interface(l, r_cell, true);
                if apply_l {
                    if let Some(li) = li {
                        rh[li] -= f[0] / dx;
                        rhu[li] -= (f[1] + src_l) / dx;
                        rhv[li] -= f[2] / dx;
                    }
                }
                if apply_r {
                    if let Some(ri) = ri {
                        rh[ri] += f[0] / dx;
                        rhu[ri] += (f[1] + src_r) / dx;
                        rhv[ri] += f[2] / dx;
                    }
                }
            }
        }

        // ---- y-direction faces (normal = y; normal momentum = hv) ----
        for c in 0..nc {
            for r in 0..=nr {
                let li = if r > 0 { Some(self.grid.idx(c, r - 1)) } else { None };
                let ri = if r < nr { Some(self.grid.idx(c, r)) } else { None };
                let ml = li.is_some_and(|i| self.masked(i));
                let mr = ri.is_some_and(|i| self.masked(i));
                let (l, r_cell, apply_l, apply_r) = match (li, ri) {
                    (Some(li), Some(ri)) => {
                        if ml && mr { continue; }
                        if mr { let ic = ry(li, 1.0); (ic, self.ghost(ic, SwBc::Wall, false), true, false) }
                        else if ml { let ic = ry(ri, -1.0); (self.ghost(ic, SwBc::Wall, false), ic, false, true) }
                        else { (ry(li, 1.0), ry(ri, -1.0), true, true) }
                    }
                    (None, Some(ri)) => { if mr { continue; } let ic = ry(ri, -1.0); (self.ghost(ic, self.bc.ymin, false), ic, false, true) }
                    (Some(li), None) => { if ml { continue; } let ic = ry(li, 1.0); (ic, self.ghost(ic, self.bc.ymax, false), true, false) }
                    (None, None) => continue,
                };
                let (f, src_l, src_r) = self.interface(l, r_cell, false);
                if apply_l {
                    if let Some(li) = li {
                        rh[li] -= f[0] / dy;
                        rhv[li] -= (f[1] + src_l) / dy;
                        rhu[li] -= f[2] / dy;
                    }
                }
                if apply_r {
                    if let Some(ri) = ri {
                        rh[ri] += f[0] / dy;
                        rhv[ri] += (f[1] + src_r) / dy;
                        rhu[ri] += f[2] / dy;
                    }
                }
            }
        }

        // ---- MUSCL centered bed-slope source (well-balancing) ----
        // The hydrostatic thrust of the bed sloping across the cell:
        //   Sc = −g·h̄·(z_high − z_low)/d  per direction (h̄ = mean face depth).
        // Zero on a flat bed (no spurious force), and exactly cancels the
        // reconstructed-face pressure imbalance for still water (C-property).
        if muscl {
            let g = self.params.gravity;
            for i in 0..n {
                if self.masked(i) {
                    continue;
                }
                let (hl, hr) = (rx(i, -1.0).h, rx(i, 1.0).h);
                rhu[i] -= g * 0.5 * (hl + hr) * sxz[i] / dx;
                let (hb, ht) = (ry(i, -1.0).h, ry(i, 1.0).h);
                rhv[i] -= g * 0.5 * (hb + ht) * syz[i] / dy;
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

    /// Clamp depths ≥ 0 and zero momentum in dry cells; solid (masked) cells are
    /// forced fully dry.
    fn positivity(&self, s: &mut SwState) {
        for i in 0..s.n_cells() {
            if self.masked(i) {
                s.h[i] = 0.0;
                s.hu[i] = 0.0;
                s.hv[i] = 0.0;
            } else if s.h[i] <= self.params.h_dry {
                s.h[i] = s.h[i].max(0.0);
                s.hu[i] = 0.0;
                s.hv[i] = 0.0;
            }
        }
    }

    /// Manning friction, semi-implicit: d(hu)/dt = −k·hu with
    /// k = g n² |U| / h^{4/3}; solved as hu ← hu / (1 + dt·k). Uses the per-cell
    /// `manning_field` (Roughness burn-in) when present, else the scalar n.
    fn friction(&self, s: &mut SwState, dt: f64) {
        let scalar_n = self.params.manning_n;
        if scalar_n <= 0.0 && self.manning_field.is_none() {
            return;
        }
        let g = self.params.gravity;
        for i in 0..s.n_cells() {
            let h = s.h[i];
            if h <= self.params.h_dry {
                continue;
            }
            let n = self.manning_field.as_ref().map_or(scalar_n, |m| m.get(i).copied().unwrap_or(scalar_n));
            if n <= 0.0 {
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

/// Minmod slope limiter: 0 if the one-sided differences disagree in sign, else
/// the smaller-magnitude one. Total-variation diminishing.
#[inline]
fn minmod(a: f64, b: f64) -> f64 {
    if a * b <= 0.0 {
        0.0
    } else if a.abs() < b.abs() {
        a
    } else {
        b
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
    fn muscl_preserves_c_property() {
        // MUSCL (order 2) must STILL keep still water flat over a bumpy bed —
        // the centered bed source must cancel the reconstructed pressure imbalance.
        let (nc, nr) = (40, 1);
        let mut zb = flat(nc, nr);
        for c in 0..nc {
            zb[c] = 0.3 * (c as f64 * 0.4).sin().abs();
        }
        let s0 = SwState::lake_at_rest(zb, 0.6);
        let grid = SwGrid::new(nc, nr, 1.0, 1.0);
        let solver = SwSolver::new(grid, SwParams { manning_n: 0.0, order: 2, ..Default::default() }, SwBoundaries::default());
        let mut s = s0.clone();
        for _ in 0..50 {
            solver.step(&mut s, 0.05);
        }
        for i in 0..s.n_cells() {
            assert!((s.h[i] + s.z_b[i] - 0.6).abs() < 1e-10, "MUSCL eta drift at {i}: {}", s.h[i] + s.z_b[i]);
            assert!(s.hu[i].abs() < 1e-9, "MUSCL spurious momentum at {i}");
        }
    }

    #[test]
    fn muscl_sharper_than_first_order_dam_break() {
        // Order 2 should resolve the Ritter dam-front sharper → smaller dam-depth
        // error than 1st order, and the front advances further (less diffusion).
        let (nc, nr) = (400, 1);
        let (dx, g, hl) = (1.0, 9.81, 10.0);
        let dam = nc / 2;
        let run = |order: u8| -> (f64, usize) {
            let mut s = SwState::dry(flat(nc, nr));
            for c in 0..dam { s.h[c] = hl; }
            let solver = SwSolver::new(
                SwGrid::new(nc, nr, dx, 1.0),
                SwParams { gravity: g, manning_n: 0.0, order, ..Default::default() },
                SwBoundaries { xmin: SwBc::Transmissive, xmax: SwBc::Transmissive, ..Default::default() },
            );
            solver.advance(&mut s, 4.0, 100000);
            let mut front = dam;
            for c in dam..nc { if s.h[c] > 0.05 { front = c; } }
            (s.h[dam], front)
        };
        let (h1, f1) = run(1);
        let (h2, f2) = run(2);
        let exact = 4.0 / 9.0 * hl;
        let (e1, e2) = ((h1 - exact).abs(), (h2 - exact).abs());
        eprintln!("1st: h_dam={h1:.4} (err {e1:.4}) front={f1} | MUSCL: h_dam={h2:.4} (err {e2:.4}) front={f2}");
        // MUSCL stays accurate at the dam and resolves the front at least as far.
        assert!(e2 < 0.2, "MUSCL dam-depth error {e2} too large");
        assert!(f2 >= f1, "MUSCL front {f2} should reach ≥ 1st-order front {f1}");
    }

    #[test]
    fn hole_mask_blocks_flow_and_stays_dry() {
        // A 1D channel with a masked (solid) cell: water released on the left must
        // NOT pass through the masked cell, which stays dry.
        let (nc, nr) = (20, 1);
        let mut s = SwState::dry(flat(nc, nr));
        for c in 0..5 { s.h[c] = 2.0; }
        let mut solver = SwSolver::new(
            SwGrid::new(nc, nr, 1.0, 1.0),
            SwParams { manning_n: 0.0, ..Default::default() },
            SwBoundaries { xmin: SwBc::Wall, xmax: SwBc::Wall, ..Default::default() },
        );
        solver.set_solid(10); // wall at cell 10
        solver.advance(&mut s, 5.0, 100000);
        assert!(s.h.iter().all(|&h| h >= 0.0 && h.is_finite()));
        assert!(s.h[10] < 1e-9, "masked cell must stay dry, got {}", s.h[10]);
        // Nothing should be downstream of the wall (cells 11..) — fully dry.
        for c in 11..nc {
            assert!(s.h[c] < 1e-6, "water leaked past the wall at {c}: {}", s.h[c]);
        }
    }

    #[test]
    fn lake_at_rest_with_solid_mask() {
        // Still water with an internal wall stays flat (C-property preserved).
        let (nc, nr) = (20, 1);
        let s0 = SwState::lake_at_rest(flat(nc, nr), 1.0);
        let mut solver = SwSolver::new(SwGrid::new(nc, nr, 1.0, 1.0), SwParams { manning_n: 0.0, ..Default::default() }, SwBoundaries::default());
        solver.set_solid(7);
        solver.set_solid(8);
        let mut s = s0.clone();
        for _ in 0..40 { solver.step(&mut s, 0.05); }
        for i in 0..s.n_cells() {
            if solver.masked(i) {
                assert!(s.h[i] < 1e-12, "masked cell wet at {i}");
            } else {
                assert!((s.h[i] - 1.0).abs() < 1e-9, "level drift at {i}: {}", s.h[i]);
                assert!(s.hu[i].abs() < 1e-9, "spurious momentum at {i}");
            }
        }
    }

    #[test]
    fn roughness_damps_flow() {
        // Higher per-cell Manning n in a downstream strip slows the front: the
        // rough run advances its wet front less far than the smooth run.
        let (nc, nr) = (120, 1);
        let front = |rough: bool| -> usize {
            let mut s = SwState::dry(flat(nc, nr));
            for c in 0..20 { s.h[c] = 3.0; }
            let mut solver = SwSolver::new(
                SwGrid::new(nc, nr, 1.0, 1.0),
                SwParams { manning_n: 0.02, ..Default::default() },
                SwBoundaries { xmin: SwBc::Wall, xmax: SwBc::Transmissive, ..Default::default() },
            );
            if rough {
                for c in 40..nc { solver.set_manning(c, 0.5); } // very rough downstream
            }
            solver.advance(&mut s, 6.0, 100000);
            let mut f = 0;
            for c in 0..nc { if s.h[c] > 0.02 { f = c; } }
            f
        };
        let smooth = front(false);
        let rough = front(true);
        assert!(rough < smooth, "rough front {rough} should lag smooth front {smooth}");
    }

    #[test]
    fn stoker_wet_dam_break_star_depth() {
        // Stoker (1957) wet-bed dam break: hL>hR>0, flat frictionless bed. The
        // self-similar solution has a constant plateau h_m (star state) between a
        // left rarefaction and a right shock. Solve h_m analytically (rarefaction
        // invariant = shock Rankine-Hugoniot) and compare the simulated plateau.
        let (nc, nr) = (600, 1);
        let (dx, g, hl, hr) = (1.0_f64, 9.81_f64, 2.0_f64, 0.5_f64);
        let dam = nc / 2;
        let t = 6.0_f64;

        // h_m root of f(h) = 2(√(g hL)−√(g h)) − (h−hR)√(g(h+hR)/(2 h hR)).
        let fstar = |h: f64| 2.0 * ((g * hl).sqrt() - (g * h).sqrt())
            - (h - hr) * (g * (h + hr) / (2.0 * h * hr)).sqrt();
        let (mut lo, mut hi) = (hr, hl);
        for _ in 0..100 {
            let mid = 0.5 * (lo + hi);
            if fstar(mid) > 0.0 { lo = mid; } else { hi = mid; }
        }
        let h_m = 0.5 * (lo + hi);
        let u_m = 2.0 * ((g * hl).sqrt() - (g * h_m).sqrt());
        let shock_speed = h_m * u_m / (h_m - hr); // RH speed into still water hR

        let mut s = SwState::dry(flat(nc, nr));
        for c in 0..nc { s.h[c] = if c < dam { hl } else { hr }; }
        let solver = SwSolver::new(
            SwGrid::new(nc, nr, dx, 1.0),
            SwParams { gravity: g, manning_n: 0.0, order: 2, ..Default::default() },
            SwBoundaries { xmin: SwBc::Transmissive, xmax: SwBc::Transmissive, ..Default::default() },
        );
        let v0 = solver.total_volume(&s);
        solver.advance(&mut s, t, 200000);

        assert!(s.h.iter().all(|&h| h >= 0.0 && h.is_finite()), "positivity");
        // Shock front: rightmost cell clearly above the (h_m+hR)/2 midpoint.
        let thresh = 0.5 * (h_m + hr);
        let mut shock = dam;
        for c in dam..nc { if s.h[c] > thresh { shock = c; } }
        let shock_x = shock as f64 * dx;
        let analytic_shock = dam as f64 * dx + shock_speed * t;
        assert!((shock_x - analytic_shock).abs() < 8.0 * dx, "shock {shock_x} vs analytic {analytic_shock}");
        // Plateau: a few cells upstream of the shock must sit at the star depth.
        let probe = shock.saturating_sub(15);
        assert!((s.h[probe] - h_m).abs() / h_m < 0.06, "plateau {} vs h_m {h_m}", s.h[probe]);
        // Downstream of the shock stays near hR; mass conserved (no boundary reached).
        assert!((s.h[shock + 20] - hr).abs() < 0.05, "downstream {} vs hR {hr}", s.h[shock + 20]);
        let v1 = solver.total_volume(&s);
        assert!((v1 - v0).abs() / v0 < 1e-3, "mass drift {}", (v1 - v0).abs() / v0);
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

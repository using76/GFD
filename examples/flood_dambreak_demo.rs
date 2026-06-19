//! Demo: 1D Ritter dam break on a flat dry bed — runs the 2D SWE solver and
//! compares against the analytic Ritter solution. Run with:
//!   cargo run --release --example flood_dambreak_demo

use gfd_fluid::shallow_water::{SwBc, SwBoundaries, SwGrid, SwParams, SwSolver, SwState};

fn main() {
    let (nc, nr) = (400usize, 1usize);
    let dx = 1.0;
    let g = 9.81;
    let hl = 10.0; // upstream depth
    let dam = nc / 2;
    let t_end = 4.0;

    // Initial condition: still water left of the dam, dry to the right.
    let mut s = SwState::dry(vec![0.0; nc * nr]);
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
    let steps = solver.advance(&mut s, t_end, 100_000);
    let v1 = solver.total_volume(&s);

    // Numeric front (last cell with appreciable depth) + dam-station depth.
    let mut front = dam;
    for c in dam..nc {
        if s.h[c] > 0.01 {
            front = c;
        }
    }
    let front_x = front as f64 * dx;
    let h_dam = s.h[dam];

    // Ritter analytic: tip at x_dam + 2√(g·hL)·t ; depth at the dam = 4/9·hL.
    let tip = dam as f64 * dx + 2.0 * (g * hl).sqrt() * t_end;
    let h_dam_exact = 4.0 / 9.0 * hl;

    println!("=== Ritter dam break (SWE solver) ===");
    println!("grid: {nc} cells, dx={dx} m | hL={hl} m | t={t_end} s | {steps} steps");
    println!("wet front:  numeric x={front_x:.1} m   analytic tip={tip:.1} m");
    println!("dam depth:  numeric h={h_dam:.3} m   analytic 4hL/9={h_dam_exact:.3} m  (err {:.1}%)",
             (h_dam - h_dam_exact).abs() / h_dam_exact * 100.0);
    println!("mass:       V0={v0:.3}  V1={v1:.3}  drift={:.2e}", (v1 - v0).abs() / v0);
    println!("positivity: min h = {:.3e}  (must be ≥ 0)", s.h.iter().cloned().fold(f64::INFINITY, f64::min));

    // Depth profile (every 20 cells) so the rarefaction → bore is visible.
    println!("\nx[m]  depth[m]");
    for c in (0..nc).step_by(20) {
        let bar = "#".repeat((s.h[c] * 3.0).round() as usize);
        println!("{:>4}  {:>6.2} {bar}", c as f64 * dx, s.h[c]);
    }
}

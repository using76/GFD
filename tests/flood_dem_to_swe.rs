//! End-to-end Phase 4 + Phase 6: read an ESRI ASCII DEM → bed field → run the
//! 2D shallow-water solver on it. Proves the DEM→grid→flood path works.

use gfd_fluid::shallow_water::{SwBc, SwBoundaries, SwGrid, SwParams, SwSolver, SwState};
use gfd_geo::dem::parse_asc;

/// A small sloping-channel DEM (.asc): 10×3 cells, bed tilts down west→east, with
/// one NODATA corner that must become an impermeable wall.
const DEM: &str = "\
ncols 10
nrows 3
xllcorner 500000.0
yllcorner 4000000.0
cellsize 5.0
NODATA_value -9999
9 8 7 6 5 4 3 2 1 0
9 8 7 6 5 4 3 2 1 0
9 8 7 6 5 -9999 3 2 1 0";

#[test]
fn dem_to_swe_flows_downslope_and_conserves_mass_until_outflow() {
    let dem = parse_asc(DEM).unwrap();
    let (nc, nr, cs, z_b) = dem.to_bed_field(None);
    assert_eq!((nc, nr), (10, 3));

    // Pond water against the high (west) wall: fill the two westmost columns to a
    // free surface of 10 m so water spills east downslope.
    let mut s = SwState::dry(z_b);
    for r in 0..nr {
        for c in 0..2 {
            let i = r * nc + c;
            s.h[i] = (10.0 - s.z_b[i]).max(0.0);
        }
    }

    let grid = SwGrid::new(nc, nr, cs, cs);
    let solver = SwSolver::new(
        grid,
        SwParams { manning_n: 0.025, ..Default::default() },
        // Open east (outflow), walls elsewhere.
        SwBoundaries { xmin: SwBc::Wall, xmax: SwBc::Transmissive, ymin: SwBc::Wall, ymax: SwBc::Wall },
    );

    let v0 = solver.total_volume(&s);
    // Short run: water spreads east but the front shouldn't reach the open east
    // edge yet, so volume is (near) conserved.
    solver.advance(&mut s, 1.0, 100000);

    // Robustness: positive, finite depths everywhere.
    assert!(s.h.iter().all(|&h| h >= 0.0 && h.is_finite()), "non-physical depth");

    // Water moved east (a previously-dry interior column is now wet).
    let wet_mid: usize = (0..nr).filter(|&r| s.h[r * nc + 4] > 1e-3).count();
    assert!(wet_mid > 0, "water did not flow downslope into mid-channel");

    // Mass conserved within tolerance before reaching the open boundary.
    let v1 = solver.total_volume(&s);
    assert!((v1 - v0).abs() / v0 < 1e-2, "mass drift {}", (v1 - v0).abs() / v0);

    // The NODATA wall cell (row 2, col 5) stays dry (impermeable high ground).
    assert!(s.h[2 * nc + 5] < 1e-6, "water entered the NODATA wall cell");
}

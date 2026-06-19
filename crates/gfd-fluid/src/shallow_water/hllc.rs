//! HLLC approximate Riemann solver for the 2D shallow-water equations, written
//! in face-normal coordinates. The (h, normal-momentum) subsystem is solved with
//! HLL wave speeds (Einfeldt estimate + dry-bed correction); the transverse
//! momentum is carried across the contact by upwinding on the mass flux — the
//! standard SWE-HLLC treatment. Positivity-preserving for h ≥ 0.

const DRY: f64 = 1.0e-12;

/// Numerical flux `[F_h, F_qn, F_qt]` for an interface with left/right states
/// given as depth `h`, normal velocity `un`, transverse velocity `ut`.
/// `qn = h·un` (normal momentum), `qt = h·ut` (transverse momentum).
pub fn hllc_normal(hl: f64, unl: f64, utl: f64, hr: f64, unr: f64, utr: f64, g: f64) -> [f64; 3] {
    if hl <= DRY && hr <= DRY {
        return [0.0, 0.0, 0.0];
    }
    let cl = (g * hl.max(0.0)).sqrt();
    let cr = (g * hr.max(0.0)).sqrt();

    // Wave-speed estimates with wet/dry handling.
    let (sl, sr);
    if hl <= DRY {
        sl = unr - 2.0 * cr; // dry left: left-going rarefaction into the dry bed
        sr = unr + cr;
    } else if hr <= DRY {
        sl = unl - cl;
        sr = unl + 2.0 * cl; // dry right
    } else {
        // Einfeldt (HLLE): combine cell estimates with a Roe-like average.
        let sq_l = hl.sqrt();
        let sq_r = hr.sqrt();
        let ubar = (unl * sq_l + unr * sq_r) / (sq_l + sq_r);
        let cbar = (g * 0.5 * (hl + hr)).sqrt();
        sl = (unl - cl).min(ubar - cbar);
        sr = (unr + cr).max(ubar + cbar);
    }

    // Physical fluxes F(U) in the normal direction.
    let fl = [hl * unl, hl * unl * unl + 0.5 * g * hl * hl, hl * unl * utl];
    let fr = [hr * unr, hr * unr * unr + 0.5 * g * hr * hr, hr * unr * utr];

    if sl >= 0.0 {
        return fl;
    }
    if sr <= 0.0 {
        return fr;
    }

    let inv = 1.0 / (sr - sl);
    // HLL for mass and normal momentum.
    let f_h = (sr * fl[0] - sl * fr[0] + sl * sr * (hr - hl)) * inv;
    let f_qn = (sr * fl[1] - sl * fr[1] + sl * sr * (hr * unr - hl * unl)) * inv;

    // Contact-wave speed → upwind the transverse velocity on the mass flux.
    let denom = hr * (unr - sr) - hl * (unl - sl);
    let s_star = if denom.abs() > DRY {
        (sl * hr * (unr - sr) - sr * hl * (unl - sl)) / denom
    } else {
        0.5 * (sl + sr)
    };
    let f_qt = if s_star >= 0.0 { f_h * utl } else { f_h * utr };

    [f_h, f_qn, f_qt]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn still_equal_states_give_pure_pressure_flux() {
        // h=2, u=v=0 both sides → mass & advective momentum 0; only ½gh² pressure.
        let g = 9.81;
        let f = hllc_normal(2.0, 0.0, 0.0, 2.0, 0.0, 0.0, g);
        assert!(f[0].abs() < 1e-12, "mass flux should be 0");
        assert!((f[1] - 0.5 * g * 4.0).abs() < 1e-9, "pressure flux ½gh²");
        assert!(f[2].abs() < 1e-12);
    }

    #[test]
    fn supersonic_left_returns_left_flux() {
        // Strongly right-moving flow (un >> c): upwind picks the left physical flux.
        let g = 9.81;
        let f = hllc_normal(1.0, 20.0, 3.0, 1.0, 20.0, 3.0, g);
        let fl = [1.0 * 20.0, 1.0 * 400.0 + 0.5 * g, 1.0 * 20.0 * 3.0];
        for k in 0..3 {
            assert!((f[k] - fl[k]).abs() < 1e-9, "comp {k}");
        }
    }

    #[test]
    fn dry_right_is_finite_and_positive_massflux() {
        // Wet left, dry right → wetting front, mass flux ≥ 0, all finite.
        let f = hllc_normal(1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 9.81);
        assert!(f.iter().all(|x| x.is_finite()));
        assert!(f[0] >= 0.0);
    }
}

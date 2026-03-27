//! Geometry analysis: volume, bounding box, distance, interference.

/// Compute volume of an SDF body using Monte Carlo integration.
pub fn compute_volume(sdf: &dyn Fn([f64; 3]) -> f64, bbox: [[f64; 3]; 2], samples: usize) -> f64 {
    let dx = bbox[1][0] - bbox[0][0];
    let dy = bbox[1][1] - bbox[0][1];
    let dz = bbox[1][2] - bbox[0][2];
    let total_volume = dx * dy * dz;
    let mut inside = 0usize;

    // Quasi-random sampling (Halton-like)
    for i in 0..samples {
        let t = i as f64 / samples as f64;
        let x = bbox[0][0] + dx * ((t * 7.0 + 0.1).fract());
        let y = bbox[0][1] + dy * ((t * 13.0 + 0.3).fract());
        let z = bbox[0][2] + dz * ((t * 29.0 + 0.7).fract());
        if sdf([x, y, z]) < 0.0 {
            inside += 1;
        }
    }

    total_volume * inside as f64 / samples as f64
}

/// Compute approximate bounding box of SDF body.
pub fn compute_bounding_box(sdf: &dyn Fn([f64; 3]) -> f64, search_range: f64, resolution: usize) -> [[f64; 3]; 2] {
    let mut min = [search_range, search_range, search_range];
    let mut max = [-search_range, -search_range, -search_range];
    let step = 2.0 * search_range / resolution as f64;
    let mut found = false;

    for i in 0..resolution {
        for j in 0..resolution {
            for k in 0..resolution {
                let x = -search_range + i as f64 * step;
                let y = -search_range + j as f64 * step;
                let z = -search_range + k as f64 * step;
                if sdf([x, y, z]) < 0.0 {
                    found = true;
                    min[0] = min[0].min(x);
                    min[1] = min[1].min(y);
                    min[2] = min[2].min(z);
                    max[0] = max[0].max(x);
                    max[1] = max[1].max(y);
                    max[2] = max[2].max(z);
                }
            }
        }
    }

    if !found { return [[0.0; 3]; 2]; }
    [min, max]
}

/// Measure minimum distance between two SDF bodies.
pub fn measure_distance(
    a: &dyn Fn([f64; 3]) -> f64,
    b: &dyn Fn([f64; 3]) -> f64,
    bbox: [[f64; 3]; 2],
    samples: usize,
) -> f64 {
    let dx = bbox[1][0] - bbox[0][0];
    let dy = bbox[1][1] - bbox[0][1];
    let dz = bbox[1][2] - bbox[0][2];
    let mut min_sum = f64::MAX;

    for i in 0..samples {
        let t = i as f64 / samples as f64;
        let x = bbox[0][0] + dx * ((t * 7.0 + 0.1).fract());
        let y = bbox[0][1] + dy * ((t * 13.0 + 0.3).fract());
        let z = bbox[0][2] + dz * ((t * 29.0 + 0.7).fract());
        let da = a([x, y, z]).abs();
        let db = b([x, y, z]).abs();
        min_sum = min_sum.min(da + db);
    }
    min_sum
}

/// Check if two SDF bodies interfere (overlap).
pub fn check_interference(
    a: &dyn Fn([f64; 3]) -> f64,
    b: &dyn Fn([f64; 3]) -> f64,
    bbox: [[f64; 3]; 2],
    samples: usize,
) -> bool {
    let dx = bbox[1][0] - bbox[0][0];
    let dy = bbox[1][1] - bbox[0][1];
    let dz = bbox[1][2] - bbox[0][2];

    for i in 0..samples {
        let t = i as f64 / samples as f64;
        let x = bbox[0][0] + dx * ((t * 7.0 + 0.1).fract());
        let y = bbox[0][1] + dy * ((t * 13.0 + 0.3).fract());
        let z = bbox[0][2] + dz * ((t * 29.0 + 0.7).fract());
        if a([x, y, z]) < 0.0 && b([x, y, z]) < 0.0 {
            return true;
        }
    }
    false
}

/// Compute surface area by sampling surface points.
pub fn compute_surface_area(sdf: &dyn Fn([f64; 3]) -> f64, bbox: [[f64; 3]; 2], resolution: usize) -> f64 {
    let dx = (bbox[1][0] - bbox[0][0]) / resolution as f64;
    let dy = (bbox[1][1] - bbox[0][1]) / resolution as f64;
    let dz = (bbox[1][2] - bbox[0][2]) / resolution as f64;
    let cell_area = dx * dy; // approximate face area
    let mut surface_cells = 0usize;

    for i in 0..resolution {
        for j in 0..resolution {
            for k in 0..resolution {
                let x = bbox[0][0] + (i as f64 + 0.5) * dx;
                let y = bbox[0][1] + (j as f64 + 0.5) * dy;
                let z = bbox[0][2] + (k as f64 + 0.5) * dz;
                let d = sdf([x, y, z]);
                if d.abs() < dx * 0.5 {
                    surface_cells += 1;
                }
            }
        }
    }

    surface_cells as f64 * cell_area
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::primitives::*;

    #[test]
    fn test_volume_sphere() {
        let r = 1.0;
        let sphere = sdf_sphere([0.0, 0.0, 0.0], r);
        let vol = compute_volume(&sphere, [[-2.0; 3], [2.0; 3]], 50000);
        let exact = 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
        assert!((vol - exact).abs() / exact < 0.1, "Volume {vol} vs exact {exact}");
    }

    #[test]
    fn test_volume_box() {
        let bx = sdf_box([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let vol = compute_volume(&bx, [[-1.0; 3], [2.0; 3]], 50000);
        assert!((vol - 1.0).abs() < 0.15, "Box volume {vol} vs 1.0");
    }

    #[test]
    fn test_bounding_box() {
        let sphere = sdf_sphere([0.0, 0.0, 0.0], 1.0);
        let bb = compute_bounding_box(&sphere, 3.0, 30);
        assert!(bb[0][0] < -0.5 && bb[1][0] > 0.5);
    }

    #[test]
    fn test_interference_yes() {
        let a = sdf_sphere([0.0, 0.0, 0.0], 1.0);
        let b = sdf_sphere([0.5, 0.0, 0.0], 1.0);
        assert!(check_interference(&a, &b, [[-3.0; 3], [3.0; 3]], 10000));
    }

    #[test]
    fn test_interference_no() {
        let a = sdf_sphere([0.0, 0.0, 0.0], 0.4);
        let b = sdf_sphere([3.0, 0.0, 0.0], 0.4);
        assert!(!check_interference(&a, &b, [[-5.0; 3], [5.0; 3]], 10000));
    }

    #[test]
    fn test_distance_separate() {
        let a = sdf_sphere([0.0, 0.0, 0.0], 1.0);
        let b = sdf_sphere([5.0, 0.0, 0.0], 1.0);
        let d = measure_distance(&a, &b, [[-2.0, -2.0, -2.0], [7.0, 2.0, 2.0]], 50000);
        assert!(d > 0.0 && d < 5.0, "Distance should be ~3, got {d}");
    }
}

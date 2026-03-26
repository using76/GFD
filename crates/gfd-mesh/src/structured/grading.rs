//! Grading (stretching) functions for non-uniform node distributions.
//!
//! Each function returns `n+1` node positions along `[0, length]`.

/// Uniform distribution: n+1 equally-spaced positions in `[0, length]`.
pub fn uniform_distribution(n: usize, length: f64) -> Vec<f64> {
    let h = length / n as f64;
    (0..=n).map(|i| i as f64 * h).collect()
}

/// Geometric distribution with prescribed first cell height and expansion ratio.
///
/// `h_i = first * ratio^(i-1)` for `i = 1..n`.
/// The positions are normalised so they span exactly `[0, length]`.
pub fn geometric_distribution(n: usize, length: f64, first: f64, ratio: f64) -> Vec<f64> {
    if n == 0 {
        return vec![0.0];
    }

    // Build raw cumulative heights
    let mut positions = Vec::with_capacity(n + 1);
    positions.push(0.0);
    let mut h = first;
    let mut cum = 0.0;
    for _ in 0..n {
        cum += h;
        positions.push(cum);
        h *= ratio;
    }

    // Scale to fit [0, length]
    let scale = length / cum;
    for p in &mut positions {
        *p *= scale;
    }
    // Ensure exact endpoints
    *positions.last_mut().unwrap() = length;
    positions
}

/// Hyperbolic-tangent distribution clustering points at both ends of `[0, length]`.
///
/// `delta` controls clustering strength: smaller => stronger clustering.
/// The formula is: `x_i = length/2 * (1 + tanh(delta * (2*i/n - 1)) / tanh(delta))`.
pub fn tanh_distribution(n: usize, length: f64, delta: f64) -> Vec<f64> {
    if n == 0 {
        return vec![0.0];
    }
    let tanh_delta = delta.tanh();
    (0..=n)
        .map(|i| {
            let xi = 2.0 * i as f64 / n as f64 - 1.0;
            length / 2.0 * (1.0 + (delta * xi).tanh() / tanh_delta)
        })
        .collect()
}

/// Bi-geometric distribution: geometric growth from the start (first cell = `first`)
/// and geometric growth from the end (last cell = `last`).
///
/// Uses a blending strategy: generate geometric from each end and blend.
/// The positions are normalised so they span exactly `[0, length]`.
pub fn bigeometric_distribution(n: usize, length: f64, first: f64, last: f64) -> Vec<f64> {
    if n == 0 {
        return vec![0.0];
    }
    if n == 1 {
        return vec![0.0, length];
    }

    // We solve for the interior distribution by blending two one-sided
    // geometric distributions.  If first == last, it is symmetric.
    //
    // Strategy: use a single geometric ratio such that the first cell has
    // height `first` and the last cell has height `last`.
    // h_0 = first, h_{n-1} = last.  We need h_{n-1} = first * r^{n-1} = last
    // => r = (last/first)^(1/(n-1))
    let ratio = if n >= 2 {
        (last / first).powf(1.0 / (n as f64 - 1.0))
    } else {
        1.0
    };

    let mut positions = Vec::with_capacity(n + 1);
    positions.push(0.0);
    let mut cum = 0.0;
    let mut h = first;
    for _ in 0..n {
        cum += h;
        positions.push(cum);
        h *= ratio;
    }

    // Scale to [0, length]
    let scale = length / cum;
    for p in &mut positions {
        *p *= scale;
    }
    *positions.last_mut().unwrap() = length;
    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform() {
        let pos = uniform_distribution(4, 2.0);
        assert_eq!(pos.len(), 5);
        assert!((pos[0] - 0.0).abs() < 1e-15);
        assert!((pos[4] - 2.0).abs() < 1e-15);
        // Uniform spacing = 0.5
        for i in 0..4 {
            let h = pos[i + 1] - pos[i];
            assert!((h - 0.5).abs() < 1e-14, "h[{}] = {}", i, h);
        }
    }

    #[test]
    fn test_geometric() {
        let pos = geometric_distribution(5, 1.0, 0.1, 1.5);
        assert_eq!(pos.len(), 6);
        assert!((pos[0] - 0.0).abs() < 1e-15);
        assert!((pos[5] - 1.0).abs() < 1e-14);
        // Each cell should be ratio times the previous
        for i in 1..5 {
            let h_prev = pos[i] - pos[i - 1];
            let h_cur = pos[i + 1] - pos[i];
            let r = h_cur / h_prev;
            assert!(
                (r - 1.5).abs() < 1e-12,
                "ratio at {} = {} (expected 1.5)",
                i,
                r
            );
        }
    }

    #[test]
    fn test_tanh_endpoints() {
        let pos = tanh_distribution(10, 5.0, 1.5);
        assert_eq!(pos.len(), 11);
        assert!((pos[0] - 0.0).abs() < 1e-15);
        assert!((pos[10] - 5.0).abs() < 1e-13);
        // Should be monotonically increasing
        for i in 0..10 {
            assert!(pos[i + 1] > pos[i], "not monotone at {}", i);
        }
    }

    #[test]
    fn test_tanh_clustering() {
        let pos = tanh_distribution(20, 1.0, 2.0);
        // First and last cells should be smaller than middle cells
        let h_first = pos[1] - pos[0];
        let h_mid = pos[11] - pos[10];
        let h_last = pos[20] - pos[19];
        assert!(
            h_first < h_mid,
            "first cell {} should be smaller than mid {}",
            h_first,
            h_mid
        );
        assert!(
            h_last < h_mid,
            "last cell {} should be smaller than mid {}",
            h_last,
            h_mid
        );
    }

    #[test]
    fn test_bigeometric() {
        let pos = bigeometric_distribution(6, 2.0, 0.1, 0.4);
        assert_eq!(pos.len(), 7);
        assert!((pos[0] - 0.0).abs() < 1e-15);
        assert!((pos[6] - 2.0).abs() < 1e-14);
        // First cell should be smaller than last cell (before scaling, but
        // relative ordering preserved)
        let h_first = pos[1] - pos[0];
        let h_last = pos[6] - pos[5];
        assert!(
            h_first < h_last,
            "first {} should be < last {}",
            h_first,
            h_last
        );
    }

    #[test]
    fn test_bigeometric_symmetric() {
        let pos = bigeometric_distribution(10, 1.0, 0.05, 0.05);
        assert_eq!(pos.len(), 11);
        // With equal first and last, distribution should be symmetric
        for i in 0..10 {
            let h_front = pos[i + 1] - pos[i];
            let h_back = pos[10 - i] - pos[10 - i - 1];
            assert!(
                (h_front - h_back).abs() < 1e-12,
                "asymmetric at {}: {} vs {}",
                i,
                h_front,
                h_back
            );
        }
    }

    #[test]
    fn test_single_cell() {
        let u = uniform_distribution(1, 3.0);
        assert_eq!(u, vec![0.0, 3.0]);

        let g = geometric_distribution(1, 3.0, 1.0, 2.0);
        assert_eq!(g.len(), 2);
        assert!((g[1] - 3.0).abs() < 1e-14);
    }
}

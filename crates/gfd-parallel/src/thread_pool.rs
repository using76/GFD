//! Thread pool configuration using Rayon.

use crate::{ParallelError, Result};

/// Configures the global Rayon thread pool with the specified number of threads.
///
/// This must be called at most once, before any parallel work is dispatched.
/// If `num_threads` is 0, Rayon will use the number of available logical CPUs.
pub fn configure_thread_pool(num_threads: usize) -> Result<()> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .map_err(|e| ParallelError::ThreadPoolError(e.to_string()))
}

/// Returns the number of threads in the current Rayon thread pool.
pub fn current_num_threads() -> usize {
    rayon::current_num_threads()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_num_threads() {
        // The global pool is already initialized by default.
        let n = current_num_threads();
        assert!(n > 0);
    }
}

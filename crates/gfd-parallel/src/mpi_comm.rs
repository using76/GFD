//! MPI communicator abstraction.
//!
//! Provides a thin abstraction over MPI communication primitives.
//! The current implementation is a single-process stub that allows the
//! solver to run without an MPI library.  When compiled with an actual
//! MPI binding (e.g., `rsmpi`), the methods should be replaced with
//! real collective/point-to-point calls.

use gfd_core::FieldSet;

/// Abstraction over an MPI communicator.
///
/// In single-process mode every operation is a no-op or returns
/// trivial values (rank 0, size 1).
#[derive(Debug, Clone)]
pub struct MpiCommunicator {
    /// Rank of this process.
    rank: usize,
    /// Total number of processes in the communicator.
    size: usize,
}

impl Default for MpiCommunicator {
    fn default() -> Self {
        Self::new()
    }
}

impl MpiCommunicator {
    /// Creates a new single-process communicator stub.
    pub fn new() -> Self {
        Self { rank: 0, size: 1 }
    }

    /// Returns the rank of the current process.
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// Returns the total number of processes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns `true` if this is the root process (rank 0).
    pub fn is_root(&self) -> bool {
        self.rank == 0
    }

    /// Barrier synchronisation — blocks until all ranks reach this point.
    ///
    /// In single-process mode this is a no-op.
    pub fn barrier(&self) {
        // No-op for single process.
    }

    /// Sends a field set to a remote rank.
    ///
    /// In single-process mode this is a no-op because there is no
    /// remote rank to send to.
    pub fn send_field(&self, _fields: &FieldSet, _dest_rank: usize, _tag: i32) {
        // No-op for single process.
    }

    /// Receives a field set from a remote rank.
    ///
    /// In single-process mode this returns `None` because there is no
    /// remote rank to receive from.
    pub fn recv_field(&self, _source_rank: usize, _tag: i32) -> Option<FieldSet> {
        // No data to receive in single-process mode.
        None
    }

    /// All-reduce a scalar value (sum) across all ranks.
    ///
    /// In single-process mode, returns the input unchanged.
    pub fn allreduce_sum_f64(&self, value: f64) -> f64 {
        value
    }

    /// All-reduce a scalar value (max) across all ranks.
    ///
    /// In single-process mode, returns the input unchanged.
    pub fn allreduce_max_f64(&self, value: f64) -> f64 {
        value
    }

    /// Broadcasts a vector of f64 from root to all ranks.
    ///
    /// In single-process mode, returns the input unchanged.
    pub fn broadcast_vec_f64(&self, data: Vec<f64>) -> Vec<f64> {
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_process_defaults() {
        let comm = MpiCommunicator::new();
        assert_eq!(comm.rank(), 0);
        assert_eq!(comm.size(), 1);
        assert!(comm.is_root());
    }

    #[test]
    fn test_barrier_noop() {
        let comm = MpiCommunicator::new();
        comm.barrier(); // should not panic
    }

    #[test]
    fn test_allreduce_identity() {
        let comm = MpiCommunicator::new();
        assert!((comm.allreduce_sum_f64(3.14) - 3.14).abs() < 1e-15);
        assert!((comm.allreduce_max_f64(2.71) - 2.71).abs() < 1e-15);
    }

    #[test]
    fn test_broadcast_identity() {
        let comm = MpiCommunicator::new();
        let data = vec![1.0, 2.0, 3.0];
        assert_eq!(comm.broadcast_vec_f64(data.clone()), data);
    }

    #[test]
    fn test_recv_returns_none() {
        let comm = MpiCommunicator::new();
        assert!(comm.recv_field(1, 0).is_none());
    }
}

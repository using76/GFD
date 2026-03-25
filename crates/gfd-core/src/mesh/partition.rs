//! Mesh partitioning data structures for parallel decomposition.

use serde::{Deserialize, Serialize};

/// Describes how a mesh is partitioned across multiple processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    /// Maps each cell index to its partition (process) index.
    pub cell_to_partition: Vec<usize>,
    /// Total number of partitions.
    pub num_partitions: usize,
}

impl Partition {
    /// Creates a new partition mapping.
    pub fn new(cell_to_partition: Vec<usize>, num_partitions: usize) -> Self {
        Self {
            cell_to_partition,
            num_partitions,
        }
    }

    /// Creates a trivial single-partition mapping for the given number of cells.
    pub fn single(num_cells: usize) -> Self {
        Self {
            cell_to_partition: vec![0; num_cells],
            num_partitions: 1,
        }
    }

    /// Returns the partition index for a given cell.
    pub fn partition_of(&self, cell_id: usize) -> Option<usize> {
        self.cell_to_partition.get(cell_id).copied()
    }

    /// Returns the cell indices belonging to the given partition.
    pub fn cells_in_partition(&self, partition_id: usize) -> Vec<usize> {
        self.cell_to_partition
            .iter()
            .enumerate()
            .filter(|(_, &p)| p == partition_id)
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns the total number of cells in the partitioned mesh.
    pub fn num_cells(&self) -> usize {
        self.cell_to_partition.len()
    }
}

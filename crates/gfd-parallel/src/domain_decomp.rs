//! Domain decomposition for parallel mesh partitioning.

use gfd_core::{Partition, UnstructuredMesh};
use crate::{ParallelError, Result};

/// Decomposes a mesh into `num_parts` partitions for parallel processing.
///
/// Uses a simple greedy approach that assigns cells to partitions
/// in contiguous blocks. For production use, this should be replaced
/// with a graph-based partitioner (e.g., METIS or Scotch).
pub fn decompose(mesh: &UnstructuredMesh, num_parts: usize) -> Result<Partition> {
    let num_cells = mesh.num_cells();

    if num_parts == 0 {
        return Err(ParallelError::InvalidPartitionCount {
            requested: 0,
            num_cells,
        });
    }

    if num_parts > num_cells {
        return Err(ParallelError::InvalidPartitionCount {
            requested: num_parts,
            num_cells,
        });
    }

    if num_parts == 1 {
        return Ok(Partition::single(num_cells));
    }

    // Simple block decomposition: divide cells evenly across partitions.
    let cells_per_part = num_cells / num_parts;
    let remainder = num_cells % num_parts;

    let mut cell_to_partition = Vec::with_capacity(num_cells);
    let mut current_part = 0;
    let mut count_in_part = 0;
    let part_size = |p: usize| -> usize {
        cells_per_part + if p < remainder { 1 } else { 0 }
    };

    for _ in 0..num_cells {
        cell_to_partition.push(current_part);
        count_in_part += 1;
        if count_in_part >= part_size(current_part) && current_part < num_parts - 1 {
            current_part += 1;
            count_in_part = 0;
        }
    }

    Ok(Partition::new(cell_to_partition, num_parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::{Cell, UnstructuredMesh};

    fn make_mesh(num_cells: usize) -> UnstructuredMesh {
        let cells: Vec<Cell> = (0..num_cells)
            .map(|i| Cell::new(i, vec![], vec![], 1.0, [0.0, 0.0, 0.0]))
            .collect();
        UnstructuredMesh::from_components(vec![], vec![], cells, vec![])
    }

    #[test]
    fn test_single_partition() {
        let mesh = make_mesh(10);
        let partition = decompose(&mesh, 1).unwrap();
        assert_eq!(partition.num_partitions, 1);
        assert!(partition.cell_to_partition.iter().all(|&p| p == 0));
    }

    #[test]
    fn test_even_decomposition() {
        let mesh = make_mesh(12);
        let partition = decompose(&mesh, 3).unwrap();
        assert_eq!(partition.num_partitions, 3);
        for part_id in 0..3 {
            assert_eq!(partition.cells_in_partition(part_id).len(), 4);
        }
    }

    #[test]
    fn test_uneven_decomposition() {
        let mesh = make_mesh(10);
        let partition = decompose(&mesh, 3).unwrap();
        assert_eq!(partition.num_partitions, 3);
        // 10 / 3 = 3 remainder 1, so first partition gets 4, rest get 3
        assert_eq!(partition.cells_in_partition(0).len(), 4);
        assert_eq!(partition.cells_in_partition(1).len(), 3);
        assert_eq!(partition.cells_in_partition(2).len(), 3);
    }

    #[test]
    fn test_zero_parts_error() {
        let mesh = make_mesh(10);
        assert!(decompose(&mesh, 0).is_err());
    }

    #[test]
    fn test_too_many_parts_error() {
        let mesh = make_mesh(3);
        assert!(decompose(&mesh, 5).is_err());
    }
}

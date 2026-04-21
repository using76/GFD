use serde::{Deserialize, Serialize};

use crate::{Shape, ShapeId, TopoError, TopoResult};

/// Stable-id arena for B-Rep shapes. Ids are never reused — removal marks
/// a slot as tombstoned so that feature re-execution keeps references valid.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ShapeArena {
    entries: Vec<Option<Shape>>,
}

impl ShapeArena {
    pub fn new() -> Self { Self::default() }

    pub fn push(&mut self, shape: Shape) -> ShapeId {
        let id = ShapeId(self.entries.len() as u32);
        self.entries.push(Some(shape));
        id
    }

    pub fn get(&self, id: ShapeId) -> TopoResult<&Shape> {
        self.entries
            .get(id.0 as usize)
            .and_then(|s| s.as_ref())
            .ok_or(TopoError::InvalidId(id))
    }

    pub fn get_mut(&mut self, id: ShapeId) -> TopoResult<&mut Shape> {
        self.entries
            .get_mut(id.0 as usize)
            .and_then(|s| s.as_mut())
            .ok_or(TopoError::InvalidId(id))
    }

    pub fn remove(&mut self, id: ShapeId) -> TopoResult<Shape> {
        let slot = self.entries.get_mut(id.0 as usize).ok_or(TopoError::InvalidId(id))?;
        slot.take().ok_or(TopoError::InvalidId(id))
    }

    pub fn len(&self) -> usize {
        self.entries.iter().filter(|s| s.is_some()).count()
    }

    pub fn is_empty(&self) -> bool { self.len() == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_geom::Point3;

    #[test]
    fn push_and_get_vertex() {
        let mut a = ShapeArena::new();
        let id = a.push(Shape::vertex(Point3::new(1.0, 2.0, 3.0)));
        match a.get(id).unwrap() {
            Shape::Vertex { point } => assert_eq!(*point, Point3::new(1.0, 2.0, 3.0)),
            _ => panic!("wrong kind"),
        }
    }
}

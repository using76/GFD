//! Pattern / array features: repeat a shape N times in linear or circular
//! arrangement. Each copy is produced through `transform_shape` and all
//! copies are wrapped in a `Shape::Compound`.

use gfd_cad_topo::{shape::Shape, ShapeArena, ShapeId, TopoResult};

use crate::transform::{rotate_shape, translate_shape};

/// Linear array: N copies of `id` along the direction `(dx, dy, dz)` with
/// unit spacing (the vector itself is the offset between consecutive copies).
/// Returns a Compound containing the original + N-1 transformed copies.
pub fn linear_array(
    arena: &mut ShapeArena,
    id: ShapeId,
    count: usize,
    dx: f64,
    dy: f64,
    dz: f64,
) -> TopoResult<ShapeId> {
    let count = count.max(1);
    let mut children = Vec::with_capacity(count);
    children.push(id);
    for i in 1..count {
        let off = i as f64;
        let t = translate_shape(arena, id, dx * off, dy * off, dz * off)?;
        children.push(t);
    }
    Ok(arena.push(Shape::Compound { children }))
}

/// 2D rectangular grid array: `count_u × count_v` copies with separate
/// spacings `(du_x, du_y, du_z)` and `(dv_x, dv_y, dv_z)` — the most common
/// CAD pattern (e.g. bolt holes on a flange, keycap grid).
pub fn rectangular_array(
    arena: &mut ShapeArena,
    id: ShapeId,
    count_u: usize,
    count_v: usize,
    du_xyz: (f64, f64, f64),
    dv_xyz: (f64, f64, f64),
) -> TopoResult<ShapeId> {
    let count_u = count_u.max(1);
    let count_v = count_v.max(1);
    let mut children = Vec::with_capacity(count_u * count_v);
    children.push(id);
    for j in 0..count_v {
        for i in 0..count_u {
            if i == 0 && j == 0 { continue; }
            let tx = du_xyz.0 * i as f64 + dv_xyz.0 * j as f64;
            let ty = du_xyz.1 * i as f64 + dv_xyz.1 * j as f64;
            let tz = du_xyz.2 * i as f64 + dv_xyz.2 * j as f64;
            let t = translate_shape(arena, id, tx, ty, tz)?;
            children.push(t);
        }
    }
    Ok(arena.push(Shape::Compound { children }))
}

/// Circular array: N copies of `id` rotated about axis through origin by
/// `total_angle_rad / count` steps (or `total_angle_rad` per copy if
/// `sweep_only_full` is false).
pub fn circular_array(
    arena: &mut ShapeArena,
    id: ShapeId,
    count: usize,
    axis: (f64, f64, f64),
    total_angle_rad: f64,
) -> TopoResult<ShapeId> {
    let count = count.max(1);
    let step = total_angle_rad / count as f64;
    let mut children = Vec::with_capacity(count);
    children.push(id);
    for i in 1..count {
        let r = rotate_shape(arena, id, axis, step * i as f64)?;
        children.push(r);
    }
    Ok(arena.push(Shape::Compound { children }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::box_solid;
    use gfd_cad_topo::{collect_by_kind, ShapeKind};

    #[test]
    fn linear_array_count_4_makes_4_solids() {
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 1.0, 1.0, 1.0).unwrap();
        let arr = linear_array(&mut a, id, 4, 2.0, 0.0, 0.0).unwrap();
        let solids = collect_by_kind(&a, arr, ShapeKind::Solid);
        assert_eq!(solids.len(), 4);
    }

    #[test]
    fn rectangular_array_3x4_makes_12_solids() {
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 0.5, 0.5, 0.5).unwrap();
        let arr = rectangular_array(&mut a, id, 3, 4, (2.0, 0.0, 0.0), (0.0, 2.0, 0.0)).unwrap();
        let solids = collect_by_kind(&a, arr, ShapeKind::Solid);
        assert_eq!(solids.len(), 12);
    }

    #[test]
    fn circular_array_360_count_6_makes_6_solids() {
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 0.5, 0.5, 0.5).unwrap();
        let arr = circular_array(&mut a, id, 6, (0.0, 0.0, 1.0), std::f64::consts::TAU).unwrap();
        let solids = collect_by_kind(&a, arr, ShapeKind::Solid);
        assert_eq!(solids.len(), 6);
    }
}

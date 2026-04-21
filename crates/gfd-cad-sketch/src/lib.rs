//! gfd-cad-sketch — 2D sketcher with a Newton-Raphson constraint solver.
//!
//! Model: points are the degrees-of-freedom. Every entity (line / circle /
//! arc) stores point indices plus optional scalar params (radius). Every
//! constraint implements [`Constraint::residuals`] which evaluates a vector
//! of scalar residuals that must be driven to zero. The solver assembles a
//! Jacobian by forward-difference and Gauss-Newton steps with Levenberg
//! damping.
//!
//! This is a minimal but working solver — sufficient for coincident / H / V /
//! distance / parallel / perpendicular constraints on line-rich sketches.
//! More aggressive DOF analysis and sparse solver plumbing will land later.

use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0 };
    pub fn new(x: f64, y: f64) -> Self { Self { x, y } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PointId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Entity {
    /// Bare point (might be referenced by other entities).
    Point(PointId),
    /// Line segment defined by its two endpoint points.
    Line { a: PointId, b: PointId },
    /// Circle with a centre point and a stored radius.
    Circle { center: PointId, radius: f64 },
    /// Arc with centre, start, and end points. Radius = |center-start|;
    /// `|center-end|` residual is exposed via `PointOnArc`-style constraints.
    Arc { center: PointId, start: PointId, end: PointId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    /// Force two points to coincide (two residuals).
    Coincident(PointId, PointId),
    /// Pin a point at a fixed absolute (x, y) (two residuals).
    Fix { point: PointId, x: f64, y: f64 },
    /// Horizontal segment: y_b − y_a = 0 (one residual).
    Horizontal { line: EntityId },
    /// Vertical segment: x_b − x_a = 0 (one residual).
    Vertical { line: EntityId },
    /// Distance between two points: |ab| − value = 0.
    Distance { a: PointId, b: PointId, value: f64 },
    /// Two lines must be parallel: cross(dir_l1, dir_l2) = 0.
    Parallel { l1: EntityId, l2: EntityId },
    /// Two lines must be perpendicular: dot(dir_l1, dir_l2) = 0.
    Perpendicular { l1: EntityId, l2: EntityId },
    /// Point lies on a line: cross((p - a), (b - a)) = 0.
    PointOnLine { point: PointId, line: EntityId },
    /// Point lies on a circle: |p - c| - r = 0.
    PointOnCircle { point: PointId, circle: EntityId },
    /// Circle radius = value.
    Radius { circle: EntityId, value: f64 },
    /// Two distances between point pairs are equal: |ab| − |cd| = 0.
    EqualLength { a1: PointId, b1: PointId, a2: PointId, b2: PointId },
    /// Angle between two lines (radians) equals `value`.
    Angle { l1: EntityId, l2: EntityId, value: f64 },
    /// Arc consistency: |center-start| = |center-end|. One residual.
    ArcClosed { arc: EntityId },
    /// Tangency between a line and a circle: distance(center, line) − radius = 0.
    TangentLineCircle { line: EntityId, circle: EntityId },
    /// Two points are symmetric about a third point (midpoint):
    /// midpoint − (a + b) / 2 = 0. Two residuals.
    Symmetric { a: PointId, b: PointId, midpoint: PointId },
    /// Arc length equals `value` (uses stored radius × angular span).
    ArcLength { arc: EntityId, value: f64 },
    /// Perpendicular distance from a point to a line equals `value`.
    DistancePointLine { point: PointId, line: EntityId, value: f64 },
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Sketch {
    pub points: Vec<Point2>,
    pub entities: Vec<Entity>,
    pub constraints: Vec<Constraint>,
}

impl Sketch {
    pub fn new() -> Self { Self::default() }

    pub fn add_point(&mut self, p: Point2) -> PointId {
        let id = PointId(self.points.len() as u32);
        self.points.push(p);
        id
    }

    pub fn add_line(&mut self, a: PointId, b: PointId) -> EntityId {
        let id = EntityId(self.entities.len() as u32);
        self.entities.push(Entity::Line { a, b });
        id
    }

    pub fn add_circle(&mut self, center: PointId, radius: f64) -> EntityId {
        let id = EntityId(self.entities.len() as u32);
        self.entities.push(Entity::Circle { center, radius });
        id
    }

    pub fn add_arc(&mut self, center: PointId, start: PointId, end: PointId) -> EntityId {
        let id = EntityId(self.entities.len() as u32);
        self.entities.push(Entity::Arc { center, start, end });
        id
    }

    fn arc_parts(&self, id: EntityId) -> SketchResult<(PointId, PointId, PointId)> {
        match self.entities.get(id.0 as usize) {
            Some(Entity::Arc { center, start, end }) => Ok((*center, *start, *end)),
            _ => Err(SketchError::NotAnArc(id)),
        }
    }

    pub fn add_constraint(&mut self, c: Constraint) { self.constraints.push(c); }

    fn line_points(&self, id: EntityId) -> SketchResult<(PointId, PointId)> {
        match self.entities.get(id.0 as usize) {
            Some(Entity::Line { a, b }) => Ok((*a, *b)),
            _ => Err(SketchError::NotAline(id)),
        }
    }

    fn circle_parts(&self, id: EntityId) -> SketchResult<(PointId, f64)> {
        match self.entities.get(id.0 as usize) {
            Some(Entity::Circle { center, radius }) => Ok((*center, *radius)),
            _ => Err(SketchError::NotACircle(id)),
        }
    }

    fn set_circle_radius(&mut self, id: EntityId, r: f64) -> SketchResult<()> {
        match self.entities.get_mut(id.0 as usize) {
            Some(Entity::Circle { radius, .. }) => { *radius = r; Ok(()) }
            _ => Err(SketchError::NotACircle(id)),
        }
    }

    fn point_xy(&self, id: PointId, dof: &[f64]) -> SketchResult<(f64, f64)> {
        let idx = id.0 as usize;
        if idx >= self.points.len() { return Err(SketchError::NotFound(id)); }
        Ok((dof[2 * idx], dof[2 * idx + 1]))
    }

    /// Evaluate all constraint residuals given a DOF vector (x0,y0,x1,y1,...).
    fn residuals(&self, dof: &[f64]) -> SketchResult<Vec<f64>> {
        let mut out = Vec::with_capacity(self.constraints.len() * 2);
        for c in &self.constraints {
            match c {
                Constraint::Coincident(a, b) => {
                    let (ax, ay) = self.point_xy(*a, dof)?;
                    let (bx, by) = self.point_xy(*b, dof)?;
                    out.push(bx - ax);
                    out.push(by - ay);
                }
                Constraint::Fix { point, x, y } => {
                    let (px, py) = self.point_xy(*point, dof)?;
                    out.push(px - x);
                    out.push(py - y);
                }
                Constraint::Horizontal { line } => {
                    let (a, b) = self.line_points(*line)?;
                    let (_, ay) = self.point_xy(a, dof)?;
                    let (_, by) = self.point_xy(b, dof)?;
                    out.push(by - ay);
                }
                Constraint::Vertical { line } => {
                    let (a, b) = self.line_points(*line)?;
                    let (ax, _) = self.point_xy(a, dof)?;
                    let (bx, _) = self.point_xy(b, dof)?;
                    out.push(bx - ax);
                }
                Constraint::Distance { a, b, value } => {
                    let (ax, ay) = self.point_xy(*a, dof)?;
                    let (bx, by) = self.point_xy(*b, dof)?;
                    let d2 = (bx - ax).powi(2) + (by - ay).powi(2);
                    out.push(d2.sqrt() - value);
                }
                Constraint::Parallel { l1, l2 } => {
                    let (a, b) = self.line_points(*l1)?;
                    let (c, d) = self.line_points(*l2)?;
                    let (ax, ay) = self.point_xy(a, dof)?;
                    let (bx, by) = self.point_xy(b, dof)?;
                    let (cx, cy) = self.point_xy(c, dof)?;
                    let (dx, dy) = self.point_xy(d, dof)?;
                    // cross product of direction vectors
                    out.push((bx - ax) * (dy - cy) - (by - ay) * (dx - cx));
                }
                Constraint::Perpendicular { l1, l2 } => {
                    let (a, b) = self.line_points(*l1)?;
                    let (c, d) = self.line_points(*l2)?;
                    let (ax, ay) = self.point_xy(a, dof)?;
                    let (bx, by) = self.point_xy(b, dof)?;
                    let (cx, cy) = self.point_xy(c, dof)?;
                    let (dx, dy) = self.point_xy(d, dof)?;
                    out.push((bx - ax) * (dx - cx) + (by - ay) * (dy - cy));
                }
                Constraint::PointOnLine { point, line } => {
                    let (a, b) = self.line_points(*line)?;
                    let (px, py) = self.point_xy(*point, dof)?;
                    let (ax, ay) = self.point_xy(a, dof)?;
                    let (bx, by) = self.point_xy(b, dof)?;
                    // 2D cross: (p - a) × (b - a)
                    out.push((px - ax) * (by - ay) - (py - ay) * (bx - ax));
                }
                Constraint::PointOnCircle { point, circle } => {
                    let (center, radius) = self.circle_parts(*circle)?;
                    let (px, py) = self.point_xy(*point, dof)?;
                    let (cx, cy) = self.point_xy(center, dof)?;
                    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                    out.push(d - radius);
                }
                Constraint::Radius { circle, value } => {
                    let (_, r) = self.circle_parts(*circle)?;
                    out.push(r - value);
                }
                Constraint::EqualLength { a1, b1, a2, b2 } => {
                    let (a1x, a1y) = self.point_xy(*a1, dof)?;
                    let (b1x, b1y) = self.point_xy(*b1, dof)?;
                    let (a2x, a2y) = self.point_xy(*a2, dof)?;
                    let (b2x, b2y) = self.point_xy(*b2, dof)?;
                    let d1 = ((b1x - a1x).powi(2) + (b1y - a1y).powi(2)).sqrt();
                    let d2 = ((b2x - a2x).powi(2) + (b2y - a2y).powi(2)).sqrt();
                    out.push(d1 - d2);
                }
                Constraint::ArcClosed { arc } => {
                    let (c, s, e) = self.arc_parts(*arc)?;
                    let (cx, cy) = self.point_xy(c, dof)?;
                    let (sx, sy) = self.point_xy(s, dof)?;
                    let (ex, ey) = self.point_xy(e, dof)?;
                    let r0 = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                    let r1 = ((ex - cx).powi(2) + (ey - cy).powi(2)).sqrt();
                    out.push(r1 - r0);
                }
                Constraint::DistancePointLine { point, line, value } => {
                    let (a, b) = self.line_points(*line)?;
                    let (px, py) = self.point_xy(*point, dof)?;
                    let (ax, ay) = self.point_xy(a, dof)?;
                    let (bx, by) = self.point_xy(b, dof)?;
                    // |cross(p-a, b-a)| / |b-a|
                    let dx = bx - ax;
                    let dy = by - ay;
                    let n = (dx * dx + dy * dy).sqrt().max(f64::EPSILON);
                    let cross = (px - ax) * dy - (py - ay) * dx;
                    out.push(cross.abs() / n - value);
                }
                Constraint::ArcLength { arc, value } => {
                    let (c, s, e) = self.arc_parts(*arc)?;
                    let (cx, cy) = self.point_xy(c, dof)?;
                    let (sx, sy) = self.point_xy(s, dof)?;
                    let (ex, ey) = self.point_xy(e, dof)?;
                    let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt().max(f64::EPSILON);
                    let a1 = (sy - cy).atan2(sx - cx);
                    let a2 = (ey - cy).atan2(ex - cx);
                    let mut span = a2 - a1;
                    while span < 0.0 { span += std::f64::consts::TAU; }
                    out.push(r * span - value);
                }
                Constraint::Symmetric { a, b, midpoint } => {
                    let (ax, ay) = self.point_xy(*a, dof)?;
                    let (bx, by) = self.point_xy(*b, dof)?;
                    let (mx, my) = self.point_xy(*midpoint, dof)?;
                    out.push(mx - 0.5 * (ax + bx));
                    out.push(my - 0.5 * (ay + by));
                }
                Constraint::TangentLineCircle { line, circle } => {
                    let (a, b) = self.line_points(*line)?;
                    let (center, radius) = self.circle_parts(*circle)?;
                    let (ax, ay) = self.point_xy(a, dof)?;
                    let (bx, by) = self.point_xy(b, dof)?;
                    let (cx, cy) = self.point_xy(center, dof)?;
                    // distance from center to the infinite line (a,b):
                    // |cross((c−a), (b−a))| / |b−a|
                    let dx = bx - ax; let dy = by - ay;
                    let cross = (cx - ax) * dy - (cy - ay) * dx;
                    let n = (dx * dx + dy * dy).sqrt().max(f64::EPSILON);
                    out.push(cross.abs() / n - radius);
                }
                Constraint::Angle { l1, l2, value } => {
                    let (a, b) = self.line_points(*l1)?;
                    let (c, d) = self.line_points(*l2)?;
                    let (ax, ay) = self.point_xy(a, dof)?;
                    let (bx, by) = self.point_xy(b, dof)?;
                    let (cx, cy) = self.point_xy(c, dof)?;
                    let (dx, dy) = self.point_xy(d, dof)?;
                    let v1x = bx - ax; let v1y = by - ay;
                    let v2x = dx - cx; let v2y = dy - cy;
                    let n1 = (v1x * v1x + v1y * v1y).sqrt().max(f64::EPSILON);
                    let n2 = (v2x * v2x + v2y * v2y).sqrt().max(f64::EPSILON);
                    let cos_val = (v1x * v2x + v1y * v2y) / (n1 * n2);
                    let target = value.cos();
                    out.push(cos_val - target);
                }
            }
        }
        Ok(out)
    }

    fn dof(&self) -> Vec<f64> {
        let mut v = Vec::with_capacity(self.points.len() * 2);
        for p in &self.points { v.push(p.x); v.push(p.y); }
        v
    }

    fn apply_dof(&mut self, dof: &[f64]) {
        for (i, p) in self.points.iter_mut().enumerate() {
            p.x = dof[2 * i];
            p.y = dof[2 * i + 1];
        }
    }

    /// Count algebraic residuals produced by the current constraint set.
    /// Used by [`Sketch::dof_status`] to classify well/under/over-constrained.
    pub fn residual_count(&self) -> usize {
        let mut n = 0;
        for c in &self.constraints {
            n += match c {
                Constraint::Coincident(..) | Constraint::Fix { .. } | Constraint::Symmetric { .. } => 2,
                Constraint::Radius { .. } | Constraint::Horizontal { .. } | Constraint::Vertical { .. }
                | Constraint::Distance { .. } | Constraint::Parallel { .. }
                | Constraint::Perpendicular { .. } | Constraint::PointOnLine { .. }
                | Constraint::PointOnCircle { .. } | Constraint::EqualLength { .. }
                | Constraint::Angle { .. } | Constraint::ArcClosed { .. }
                | Constraint::TangentLineCircle { .. } | Constraint::ArcLength { .. }
                | Constraint::DistancePointLine { .. } => 1,
            };
        }
        n
    }

    /// Report the DOF status of the sketch. `dof - residual_count` gives a
    /// rough indication of whether the sketch is well-, under- or
    /// over-constrained — not a substitute for symbolic rank analysis but
    /// useful as a GUI hint.
    pub fn dof_status(&self) -> DofStatus {
        let dof = self.points.len() * 2;
        let residuals = self.residual_count();
        if residuals > dof {
            DofStatus::OverConstrained { dof, residuals }
        } else if residuals < dof {
            DofStatus::UnderConstrained { dof, residuals }
        } else {
            DofStatus::WellConstrained { dof }
        }
    }

    /// Solve the constraint system via damped Gauss-Newton.
    ///
    /// Returns the L2 norm of the final residual vector. A value below
    /// `tolerance` means the sketch is fully satisfied.
    pub fn solve(&mut self, tolerance: f64, max_iters: usize) -> SketchResult<f64> {
        // Pre-apply algebraic-only constraints (Radius) directly on entities
        // so they produce zero residual during Newton iteration.
        let radius_updates: Vec<(EntityId, f64)> = self.constraints.iter().filter_map(|c| match c {
            Constraint::Radius { circle, value } => Some((*circle, *value)),
            _ => None,
        }).collect();
        for (c, r) in radius_updates { self.set_circle_radius(c, r)?; }

        let n_dof = self.points.len() * 2;
        if n_dof == 0 { return Ok(0.0); }
        let mut dof = self.dof();
        let mut last_norm = f64::INFINITY;

        for _ in 0..max_iters {
            let r = DVector::from_vec(self.residuals(&dof)?);
            let norm = r.norm();
            if norm < tolerance {
                self.apply_dof(&dof);
                return Ok(norm);
            }
            // Forward-difference Jacobian
            let m = r.len();
            let mut jac = DMatrix::<f64>::zeros(m, n_dof);
            let h = 1.0e-7_f64.max(1.0e-9 * norm.max(1.0));
            for j in 0..n_dof {
                let save = dof[j];
                dof[j] = save + h;
                let r_plus = DVector::from_vec(self.residuals(&dof)?);
                dof[j] = save;
                for i in 0..m {
                    jac[(i, j)] = (r_plus[i] - r[i]) / h;
                }
            }
            // Gauss-Newton with Levenberg damping: (Jᵀ J + λI) δ = -Jᵀ r
            let lambda = 1.0e-6 + 1.0e-3 * norm;
            let jt = jac.transpose();
            let a = &jt * &jac + lambda * DMatrix::<f64>::identity(n_dof, n_dof);
            let b = -(&jt * &r);
            let delta = a.lu().solve(&b).ok_or(SketchError::SolveFailed)?;
            for j in 0..n_dof { dof[j] += delta[j]; }
            if (norm - last_norm).abs() < tolerance * 1.0e-3 { break; }
            last_norm = norm;
        }

        let final_norm = DVector::from_vec(self.residuals(&dof)?).norm();
        self.apply_dof(&dof);
        if final_norm > tolerance { Err(SketchError::NoConvergence(final_norm)) } else { Ok(final_norm) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DofStatus {
    UnderConstrained { dof: usize, residuals: usize },
    WellConstrained { dof: usize },
    OverConstrained { dof: usize, residuals: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum SketchError {
    #[error("point {0:?} not found")]
    NotFound(PointId),
    #[error("entity {0:?} is not a line")]
    NotAline(EntityId),
    #[error("entity {0:?} is not a circle")]
    NotACircle(EntityId),
    #[error("entity {0:?} is not an arc")]
    NotAnArc(EntityId),
    #[error("linear solve failed (singular system)")]
    SolveFailed,
    #[error("constraint solver did not converge (final residual {0})")]
    NoConvergence(f64),
}

pub type SketchResult<T> = Result<T, SketchError>;

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn coincident_two_points() {
        let mut sk = Sketch::new();
        let a = sk.add_point(Point2::new(0.0, 0.0));
        let b = sk.add_point(Point2::new(1.0, 1.0));
        sk.add_constraint(Constraint::Fix { point: a, x: 0.0, y: 0.0 });
        sk.add_constraint(Constraint::Coincident(a, b));
        let norm = sk.solve(1.0e-8, 50).unwrap();
        assert!(norm < 1.0e-6);
        assert_abs_diff_eq!(sk.points[b.0 as usize].x, 0.0, epsilon = 1e-6);
        assert_abs_diff_eq!(sk.points[b.0 as usize].y, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn distance_between_points() {
        let mut sk = Sketch::new();
        let a = sk.add_point(Point2::new(0.0, 0.0));
        let b = sk.add_point(Point2::new(0.5, 0.0));
        sk.add_constraint(Constraint::Fix { point: a, x: 0.0, y: 0.0 });
        sk.add_constraint(Constraint::Distance { a, b, value: 2.0 });
        let norm = sk.solve(1.0e-8, 100).unwrap();
        assert!(norm < 1.0e-6);
        let d = (sk.points[b.0 as usize].x.powi(2) + sk.points[b.0 as usize].y.powi(2)).sqrt();
        assert_abs_diff_eq!(d, 2.0, epsilon = 1.0e-6);
    }

    #[test]
    fn horizontal_line() {
        let mut sk = Sketch::new();
        let a = sk.add_point(Point2::new(0.0, 0.0));
        let b = sk.add_point(Point2::new(1.0, 0.5));
        let l = sk.add_line(a, b);
        sk.add_constraint(Constraint::Fix { point: a, x: 0.0, y: 0.0 });
        sk.add_constraint(Constraint::Horizontal { line: l });
        let norm = sk.solve(1.0e-8, 50).unwrap();
        assert!(norm < 1.0e-6);
        assert_abs_diff_eq!(sk.points[b.0 as usize].y, 0.0, epsilon = 1.0e-6);
    }

    #[test]
    fn point_on_line() {
        let mut sk = Sketch::new();
        let a = sk.add_point(Point2::new(0.0, 0.0));
        let b = sk.add_point(Point2::new(2.0, 2.0));
        let p = sk.add_point(Point2::new(0.5, 1.5));
        sk.add_constraint(Constraint::Fix { point: a, x: 0.0, y: 0.0 });
        sk.add_constraint(Constraint::Fix { point: b, x: 2.0, y: 2.0 });
        let l = sk.add_line(a, b);
        sk.add_constraint(Constraint::PointOnLine { point: p, line: l });
        let norm = sk.solve(1.0e-8, 100).unwrap();
        assert!(norm < 1.0e-6);
        // Now p should satisfy y ≈ x.
        let pp = sk.points[p.0 as usize];
        assert_abs_diff_eq!(pp.x, pp.y, epsilon = 1.0e-5);
    }

    #[test]
    fn radius_and_point_on_circle() {
        let mut sk = Sketch::new();
        let c = sk.add_point(Point2::new(0.0, 0.0));
        let p = sk.add_point(Point2::new(0.2, 0.2));
        sk.add_constraint(Constraint::Fix { point: c, x: 0.0, y: 0.0 });
        let circle = sk.add_circle(c, 1.0);
        sk.add_constraint(Constraint::Radius { circle, value: 2.0 });
        sk.add_constraint(Constraint::PointOnCircle { point: p, circle });
        let norm = sk.solve(1.0e-8, 100).unwrap();
        assert!(norm < 1.0e-6);
        let pp = sk.points[p.0 as usize];
        let dist = (pp.x.powi(2) + pp.y.powi(2)).sqrt();
        assert_abs_diff_eq!(dist, 2.0, epsilon = 1.0e-5);
    }

    #[test]
    fn equal_length() {
        let mut sk = Sketch::new();
        let a1 = sk.add_point(Point2::new(0.0, 0.0));
        let b1 = sk.add_point(Point2::new(3.0, 0.0));
        let a2 = sk.add_point(Point2::new(0.0, 5.0));
        let b2 = sk.add_point(Point2::new(1.0, 5.0));
        sk.add_constraint(Constraint::Fix { point: a1, x: 0.0, y: 0.0 });
        sk.add_constraint(Constraint::Fix { point: b1, x: 3.0, y: 0.0 });
        sk.add_constraint(Constraint::Fix { point: a2, x: 0.0, y: 5.0 });
        sk.add_constraint(Constraint::EqualLength { a1, b1, a2, b2 });
        let norm = sk.solve(1.0e-8, 100).unwrap();
        assert!(norm < 1.0e-6);
        let bb = sk.points[b2.0 as usize];
        let d2 = ((bb.x - 0.0).powi(2) + (bb.y - 5.0).powi(2)).sqrt();
        assert_abs_diff_eq!(d2, 3.0, epsilon = 1.0e-5);
    }

    #[test]
    fn distance_point_line_drives_perpendicular_offset() {
        let mut sk = Sketch::new();
        let a = sk.add_point(Point2::new(0.0, 0.0));
        let b = sk.add_point(Point2::new(1.0, 0.0));
        let p = sk.add_point(Point2::new(0.5, 0.1)); // close to the line
        sk.add_constraint(Constraint::Fix { point: a, x: 0.0, y: 0.0 });
        sk.add_constraint(Constraint::Fix { point: b, x: 1.0, y: 0.0 });
        let l = sk.add_line(a, b);
        sk.add_constraint(Constraint::DistancePointLine { point: p, line: l, value: 2.0 });
        let norm = sk.solve(1.0e-8, 200).unwrap();
        assert!(norm < 1e-6);
        // Line is on the x-axis; the constraint pushes p to y = ±2.
        let pp = sk.points[p.0 as usize];
        assert_abs_diff_eq!(pp.y.abs(), 2.0, epsilon = 1e-5);
    }

    #[test]
    fn arc_closed_drives_endpoint_onto_radius() {
        let mut sk = Sketch::new();
        let c = sk.add_point(Point2::new(0.0, 0.0));
        let s = sk.add_point(Point2::new(1.0, 0.0));
        let e = sk.add_point(Point2::new(0.3, 0.2)); // too close
        sk.add_constraint(Constraint::Fix { point: c, x: 0.0, y: 0.0 });
        sk.add_constraint(Constraint::Fix { point: s, x: 1.0, y: 0.0 });
        let arc = sk.add_arc(c, s, e);
        sk.add_constraint(Constraint::ArcClosed { arc });
        let norm = sk.solve(1.0e-8, 100).unwrap();
        assert!(norm < 1.0e-6);
        let p = sk.points[e.0 as usize];
        let r = (p.x.powi(2) + p.y.powi(2)).sqrt();
        assert_abs_diff_eq!(r, 1.0, epsilon = 1.0e-5);
    }

    #[test]
    fn dof_status_under_over_well() {
        let mut sk = Sketch::new();
        let a = sk.add_point(Point2::new(0.0, 0.0));
        // 1 point, 2 DOF, no constraints → under.
        match sk.dof_status() {
            DofStatus::UnderConstrained { dof: 2, residuals: 0 } => {}
            other => panic!("expected under, got {:?}", other),
        }
        sk.add_constraint(Constraint::Fix { point: a, x: 0.0, y: 0.0 });
        // 2 DOF, 2 residuals → well.
        assert!(matches!(sk.dof_status(), DofStatus::WellConstrained { dof: 2 }));
        sk.add_constraint(Constraint::Horizontal { line: EntityId(0) });
        // 2 DOF, 3 residuals → over.
        assert!(matches!(sk.dof_status(), DofStatus::OverConstrained { .. }));
    }

    #[test]
    fn symmetric_reflects_across_midpoint() {
        let mut sk = Sketch::new();
        let a = sk.add_point(Point2::new(1.0, 1.0));
        let b = sk.add_point(Point2::new(2.0, 0.5));
        let m = sk.add_point(Point2::new(0.0, 0.0));
        sk.add_constraint(Constraint::Fix { point: a, x: 1.0, y: 1.0 });
        sk.add_constraint(Constraint::Fix { point: m, x: 0.0, y: 0.0 });
        sk.add_constraint(Constraint::Symmetric { a, b, midpoint: m });
        let norm = sk.solve(1.0e-8, 100).unwrap();
        assert!(norm < 1.0e-6);
        let bp = sk.points[b.0 as usize];
        // If a=(1,1) and midpoint=(0,0), then b must be (-1,-1).
        assert_abs_diff_eq!(bp.x, -1.0, epsilon = 1.0e-5);
        assert_abs_diff_eq!(bp.y, -1.0, epsilon = 1.0e-5);
    }

    #[test]
    fn tangent_line_to_unit_circle() {
        let mut sk = Sketch::new();
        let c = sk.add_point(Point2::new(0.0, 0.0));
        sk.add_constraint(Constraint::Fix { point: c, x: 0.0, y: 0.0 });
        let circle = sk.add_circle(c, 1.0);
        // A horizontal line, both endpoints slightly above the circle.
        // Only x is pinned; y is free on both, plus Horizontal ties them.
        let a = sk.add_point(Point2::new(-2.0, 0.6));
        let b = sk.add_point(Point2::new(2.0, 0.6));
        let l = sk.add_line(a, b);
        sk.add_constraint(Constraint::Horizontal { line: l });
        sk.add_constraint(Constraint::TangentLineCircle { line: l, circle });
        let norm = sk.solve(1.0e-8, 200).unwrap();
        assert!(norm < 1.0e-5, "residual {}", norm);
        // Perpendicular distance from origin to the horizontal line is |y|,
        // so |a.y| should be 1 (the circle's radius).
        let ay = sk.points[a.0 as usize].y;
        assert_abs_diff_eq!(ay.abs(), 1.0, epsilon = 1.0e-5);
    }

    #[test]
    fn perpendicular_lines() {
        let mut sk = Sketch::new();
        let p0 = sk.add_point(Point2::new(0.0, 0.0));
        let p1 = sk.add_point(Point2::new(1.0, 0.0));
        let p2 = sk.add_point(Point2::new(0.0, 0.0));
        let p3 = sk.add_point(Point2::new(1.0, 0.2));   // not quite perpendicular
        sk.add_constraint(Constraint::Fix { point: p0, x: 0.0, y: 0.0 });
        sk.add_constraint(Constraint::Fix { point: p1, x: 1.0, y: 0.0 });
        sk.add_constraint(Constraint::Coincident(p0, p2));
        let l1 = sk.add_line(p0, p1);
        let l2 = sk.add_line(p2, p3);
        sk.add_constraint(Constraint::Perpendicular { l1, l2 });
        let norm = sk.solve(1.0e-8, 100).unwrap();
        assert!(norm < 1.0e-6, "residual {}", norm);
        // l2 direction should now be (0, ±something) — i.e. x ≈ 0.
        assert_abs_diff_eq!(sk.points[p3.0 as usize].x, 0.0, epsilon = 1.0e-5);
    }
}

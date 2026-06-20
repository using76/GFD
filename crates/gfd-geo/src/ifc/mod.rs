//! Native IFC4 reader (BIM) — FLOOD_PLAN Phase 3. IFC shares the ISO 10303-21
//! "STEP physical file" syntax, so this tokenizes `#id=IFCTYPE(args);` records
//! and extracts the building **envelope** (walls/slabs/columns/beams/roofs/
//! proxies) as 2D footprint polygons + heights — exactly what flood building
//! burn-in (Phase 5) needs. Length units are normalized to metres.
//!
//! Covered geometry: `IfcExtrudedAreaSolid` over `IfcRectangleProfileDef` and
//! `IfcArbitraryClosedProfileDef`(IfcPolyline), positioned by the element's
//! `IfcLocalPlacement` chain (translation). Triangulated/Brep/faceted reps,
//! profile rotation, and `IfcMapConversion` georeferencing are follow-ups.

use std::collections::HashMap;
use std::path::Path;

use crate::{GeoError, GeoResult};

mod parse;
use parse::{refs_in, Record, Store};

/// One extracted envelope element: its IFC type, name, and footprint polygon(s)
/// in world XY (metres), with the extrusion base elevation and height.
#[derive(Debug, Clone)]
pub struct IfcElement {
    pub id: u32,
    pub ifc_type: String,
    pub name: String,
    /// Footprint loops (world XY, metres). Usually one outer loop per solid.
    pub footprints: Vec<Vec<[f64; 2]>>,
    /// Base elevation (z of the extrusion start, metres).
    pub base_z: f64,
    /// Extrusion height (metres).
    pub height: f64,
    /// Containing building storey name (IfcRelContainedInSpatialStructure), if any.
    pub storey_name: Option<String>,
    /// Containing storey elevation (metres); 0 if unknown.
    pub storey_elevation: f64,
}

/// A building storey in the spatial hierarchy.
#[derive(Debug, Clone)]
pub struct StoreyInfo {
    pub id: u32,
    pub name: String,
    pub elevation: f64,
    pub element_count: usize,
}

/// Parsed IFC building model: envelope elements + the length→metre scale used.
#[derive(Debug, Clone)]
pub struct IfcModel {
    pub length_scale: f64,
    pub elements: Vec<IfcElement>,
    /// Spatial hierarchy: building storeys sorted by elevation (ascending).
    pub storeys: Vec<StoreyInfo>,
}

const ENVELOPE: &[&str] = &[
    "IFCWALL", "IFCWALLSTANDARDCASE", "IFCSLAB", "IFCCOLUMN", "IFCBEAM",
    "IFCROOF", "IFCBUILDINGELEMENTPROXY", "IFCFOOTING", "IFCPLATE",
];

/// Read + parse an IFC file into an [`IfcModel`].
pub fn read_ifc(path: &Path) -> GeoResult<IfcModel> {
    let text = std::fs::read_to_string(path)?;
    parse_ifc(&text)
}

/// Parse IFC text into an [`IfcModel`].
pub fn parse_ifc(text: &str) -> GeoResult<IfcModel> {
    let store = Store::parse(text)?;
    let length_scale = detect_length_scale(&store);

    let map = detect_map_conversion(&store);
    // Spatial hierarchy: storey id → (name, elevation) and element id → storey id.
    let storey_info = build_storey_map(&store, length_scale);
    let elem_storey = build_element_to_storey_map(&store);

    let mut elements = Vec::new();
    for (&id, rec) in store.iter() {
        if !ENVELOPE.contains(&rec.ty.as_str()) {
            continue;
        }
        if let Some(el) = extract_element(&store, id, rec, length_scale, map.as_ref(), &storey_info, &elem_storey) {
            if !el.footprints.is_empty() {
                elements.push(el);
            }
        }
    }
    elements.sort_by_key(|e| e.id);

    // Storey summary, sorted by elevation, with element counts. Match by storey
    // *id* (not name) so two storeys sharing a name aren't double-counted.
    let mut storeys: Vec<StoreyInfo> = storey_info.iter().map(|(&id, (name, elev))| {
        let element_count = elements.iter().filter(|e| elem_storey.get(&e.id) == Some(&id)).count();
        StoreyInfo { id, name: name.clone(), elevation: *elev, element_count }
    }).collect();
    storeys.sort_by(|a, b| a.elevation.partial_cmp(&b.elevation).unwrap_or(std::cmp::Ordering::Equal));

    Ok(IfcModel { length_scale, elements, storeys })
}

/// Map IfcBuildingStorey id → (name, absolute elevation in metres).
fn build_storey_map(store: &Store, scale: f64) -> HashMap<u32, (String, f64)> {
    let mut map = HashMap::new();
    for (&id, rec) in store.iter() {
        if rec.ty == "IFCBUILDINGSTOREY" {
            let name = rec.string_arg(2).unwrap_or_else(|| format!("Storey-{id}"));
            // Elevation: prefer the placement Z; fall back to the Elevation attr (last float).
            let placement = rec.refs().iter().find(|&&r| store.ty_of(r) == Some("IFCLOCALPLACEMENT")).copied();
            let elev = placement
                .map(|p| placement_world(store, p, 0).2 * scale)
                .filter(|z| z.abs() > 1e-12)
                .or_else(|| rec.floats().last().map(|e| e * scale))
                .unwrap_or(0.0);
            map.insert(id, (name, elev));
        }
    }
    map
}

/// Map element id → containing storey id via IfcRelContainedInSpatialStructure.
/// Arg layout: (GlobalId, OwnerHistory, Name, Desc, RelatedElements, RelatingStructure).
fn build_element_to_storey_map(store: &Store) -> HashMap<u32, u32> {
    let mut map = HashMap::new();
    for rec in store.values() {
        if rec.ty != "IFCRELCONTAINEDINSPATIALSTRUCTURE" {
            continue;
        }
        let fields = parse::split_top_level(&rec.args);
        if fields.len() < 6 {
            continue;
        }
        let related = refs_in(&fields[4]); // RelatedElements = (#e1, #e2, ...)
        let structure = refs_in(&fields[5]); // RelatingStructure = #storey
        if let Some(&storey) = structure.first() {
            for e in related {
                map.insert(e, storey);
            }
        }
    }
    map
}

/// IFC4 `IfcMapConversion` — maps engineering (local) coords to the projected map
/// CRS: (E,N) origin + a rotation from the X-axis direction + uniform scale.
#[derive(Clone, Copy)]
struct MapConversion {
    e: f64,
    n: f64,
    cos: f64,
    sin: f64,
    scale: f64,
}

impl MapConversion {
    fn apply(&self, x: f64, y: f64) -> [f64; 2] {
        [
            self.e + self.scale * (x * self.cos - y * self.sin),
            self.n + self.scale * (x * self.sin + y * self.cos),
        ]
    }
}

fn detect_map_conversion(store: &Store) -> Option<MapConversion> {
    for rec in store.values() {
        if rec.ty == "IFCMAPCONVERSION" {
            // floats (refs/strings skipped): Eastings, Northings, OrthogonalHeight,
            // XAxisAbscissa, XAxisOrdinate, [Scale].
            let f = rec.floats();
            if f.len() >= 5 {
                let (ax, ay) = (f[3], f[4]);
                let mag = (ax * ax + ay * ay).sqrt();
                let (cos, sin) = if mag > 1e-9 { (ax / mag, ay / mag) } else { (1.0, 0.0) };
                let scale = f.get(5).copied().filter(|&s| s != 0.0).unwrap_or(1.0);
                return Some(MapConversion { e: f[0], n: f[1], cos, sin, scale });
            }
        }
    }
    None
}

/// Length unit → metres. Scans IFCSIUNIT(.LENGTHUNIT.) for a MILLI/CENTI prefix.
fn detect_length_scale(store: &Store) -> f64 {
    for rec in store.values() {
        if rec.ty == "IFCSIUNIT" && rec.args.to_uppercase().contains(".LENGTHUNIT.") {
            let a = rec.args.to_uppercase();
            if a.contains(".MILLI.") { return 0.001; }
            if a.contains(".CENTI.") { return 0.01; }
            if a.contains(".KILO.") { return 1000.0; }
            return 1.0; // .METRE. (no prefix)
        }
    }
    1.0
}

/// Build an element from its product record: find its placement (translation)
/// and representation's extruded solids → footprints + height.
fn extract_element(
    store: &Store,
    id: u32,
    rec: &Record,
    scale: f64,
    map: Option<&MapConversion>,
    storey_info: &HashMap<u32, (String, f64)>,
    elem_storey: &HashMap<u32, u32>,
) -> Option<IfcElement> {
    let refs = rec.refs();
    let name = rec.string_arg(2).unwrap_or_default();

    // Among the element's refs, identify the placement and the shape.
    let mut placement = None;
    let mut shape = None;
    for r in &refs {
        match store.ty_of(*r) {
            Some("IFCLOCALPLACEMENT") => placement = Some(*r),
            Some("IFCPRODUCTDEFINITIONSHAPE") => shape = Some(*r),
            _ => {}
        }
    }
    let (ox, oy, oz) = placement.map(|p| placement_world(store, p, 0)).unwrap_or((0.0, 0.0, 0.0));
    let shape = shape?;

    // Walk ProductDefinitionShape → ShapeRepresentation(s) → items.
    let mut footprints = Vec::new();
    let mut height = 0.0_f64;
    let mut base_z = oz * scale;
    for srep in store.get(shape)?.refs() {
        if store.ty_of(srep) != Some("IFCSHAPEREPRESENTATION") {
            continue;
        }
        for item in store.get(srep).map(|r| r.refs()).unwrap_or_default() {
            match store.ty_of(item) {
                Some("IFCEXTRUDEDAREASOLID") => {
                    if let Some((loop2d, h, z0)) = extruded_footprint(store, item, scale) {
                        let world: Vec<[f64; 2]> = loop2d.iter().map(|p| [p[0] + ox * scale, p[1] + oy * scale]).collect();
                        footprints.push(world);
                        height = height.max(h);
                        base_z = base_z.min(oz * scale + z0);
                    }
                }
                // IFC4 tessellated geometry → convex-hull plan outline.
                Some("IFCTRIANGULATEDFACESET") | Some("IFCPOLYGONALFACESET") => {
                    if let Some((hull, zlo, zhi)) = faceset_footprint(store, item, scale) {
                        footprints.push(hull.iter().map(|p| [p[0] + ox * scale, p[1] + oy * scale]).collect());
                        height = height.max(zhi - zlo);
                        base_z = base_z.min(oz * scale + zlo);
                    }
                }
                // Faceted B-rep (IfcClosedShell of IfcFace/IfcPolyLoop) → hull outline.
                Some("IFCFACETEDBREP") => {
                    if let Some((hull, zlo, zhi)) = faceted_brep_footprint(store, item, scale) {
                        footprints.push(hull.iter().map(|p| [p[0] + ox * scale, p[1] + oy * scale]).collect());
                        height = height.max(zhi - zlo);
                        base_z = base_z.min(oz * scale + zlo);
                    }
                }
                _ => {}
            }
        }
    }
    if footprints.is_empty() {
        return None;
    }
    // Apply IfcMapConversion → project footprints into the map (DEM) CRS.
    if let Some(m) = map {
        for fp in &mut footprints {
            for p in fp.iter_mut() {
                *p = m.apply(p[0], p[1]);
            }
        }
    }
    let (storey_name, storey_elevation) = elem_storey
        .get(&id)
        .and_then(|sid| storey_info.get(sid))
        .map(|(n, e)| (Some(n.clone()), *e))
        .unwrap_or((None, 0.0));
    Some(IfcElement { id, ifc_type: rec.ty.clone(), name, footprints, base_z, height, storey_name, storey_elevation })
}

/// Footprint of an IfcFacetedBrep: walk Outer(IfcClosedShell) → IfcFace →
/// IfcFaceOuterBound/InnerBound → IfcPolyLoop, collect all XY points, and return
/// the convex hull + z-extent (same shape as `faceset_footprint`).
fn faceted_brep_footprint(store: &Store, brep: u32, scale: f64) -> Option<(Vec<[f64; 2]>, f64, f64)> {
    let rec = store.get(brep)?;
    let shell = *rec.refs().first()?; // Outer: IfcClosedShell
    let shell_rec = store.get(shell)?;
    if shell_rec.ty != "IFCCLOSEDSHELL" {
        return None;
    }
    let mut pts2d = Vec::new();
    let mut zlo = f64::INFINITY;
    let mut zhi = f64::NEG_INFINITY;
    for face in shell_rec.refs() {
        let Some(face_rec) = store.get(face) else { continue };
        if face_rec.ty != "IFCFACE" {
            continue;
        }
        for bound in face_rec.refs() {
            let Some(bound_rec) = store.get(bound) else { continue };
            if bound_rec.ty != "IFCFACEOUTERBOUND" && bound_rec.ty != "IFCFACEBOUND" {
                continue;
            }
            if let Some(&loop_id) = bound_rec.refs().first() {
                if let Some(pts) = polyloop_points(store, loop_id, scale) {
                    for p in pts {
                        pts2d.push([p[0], p[1]]);
                        zlo = zlo.min(p[2]);
                        zhi = zhi.max(p[2]);
                    }
                }
            }
        }
    }
    let hull = convex_hull(&pts2d);
    if hull.len() < 3 {
        None
    } else {
        Some((hull, zlo, zhi))
    }
}

/// [x,y,z] points (metres) of an IfcPolyLoop; None for edge/vertex loops.
fn polyloop_points(store: &Store, loop_id: u32, scale: f64) -> Option<Vec<[f64; 3]>> {
    let rec = store.get(loop_id)?;
    if rec.ty != "IFCPOLYLOOP" {
        return None;
    }
    let pts: Vec<[f64; 3]> = rec.refs().iter().map(|&p| {
        let c = cartesian_point(store, p);
        [c[0] * scale, c[1] * scale, c[2] * scale]
    }).collect();
    if pts.len() < 3 {
        None
    } else {
        Some(pts)
    }
}

/// Accumulated translation of an IfcLocalPlacement chain (metres-unscaled local
/// coords; caller applies the unit scale). Bounded recursion guards cycles.
fn placement_world(store: &Store, placement: u32, depth: usize) -> (f64, f64, f64) {
    if depth > 64 {
        return (0.0, 0.0, 0.0);
    }
    let Some(rec) = store.get(placement) else { return (0.0, 0.0, 0.0) };
    let refs = rec.refs();
    // IfcLocalPlacement(PlacementRelTo, RelativePlacement)
    let mut parent = (0.0, 0.0, 0.0);
    let mut local = (0.0, 0.0, 0.0);
    for r in &refs {
        match store.ty_of(*r) {
            Some("IFCLOCALPLACEMENT") => parent = placement_world(store, *r, depth + 1),
            Some("IFCAXIS2PLACEMENT3D") | Some("IFCAXIS2PLACEMENT2D") => local = axis_origin(store, *r),
            _ => {}
        }
    }
    (parent.0 + local.0, parent.1 + local.1, parent.2 + local.2)
}

/// Origin (x,y,z) of an IfcAxis2Placement via its IfcCartesianPoint location.
fn axis_origin(store: &Store, placement: u32) -> (f64, f64, f64) {
    let Some(rec) = store.get(placement) else { return (0.0, 0.0, 0.0) };
    if let Some(&loc) = rec.refs().first() {
        let p = cartesian_point(store, loc);
        return (p[0], p[1], p[2]);
    }
    (0.0, 0.0, 0.0)
}

fn cartesian_point(store: &Store, id: u32) -> [f64; 3] {
    let mut p = [0.0; 3];
    if let Some(rec) = store.get(id) {
        let f = rec.floats();
        for k in 0..3.min(f.len()) {
            p[k] = f[k];
        }
    }
    p
}

/// Footprint polygon (local XY, scaled to metres), extrusion height, and z-start
/// of an IfcExtrudedAreaSolid(SweptArea, Position, ExtrudedDirection, Depth).
fn extruded_footprint(store: &Store, solid: u32, scale: f64) -> Option<(Vec<[f64; 2]>, f64, f64)> {
    let rec = store.get(solid)?;
    let refs = rec.refs();
    let profile = *refs.first()?; // SweptArea
    // Position (IfcAxis2Placement3D) gives the extrusion base origin.
    let pos = refs.iter().find(|&&r| store.ty_of(r) == Some("IFCAXIS2PLACEMENT3D"));
    let (px, py, pz) = pos.map(|&p| axis_origin(store, p)).unwrap_or((0.0, 0.0, 0.0));
    let depth = rec.floats().last().copied().unwrap_or(0.0) * scale;
    let loop_local = profile_loop(store, profile, scale)?;
    let loop_world = loop_local.iter().map(|p| [p[0] + px * scale, p[1] + py * scale]).collect();
    Some((loop_world, depth.abs(), pz * scale))
}

/// 2D loop of a profile def (rectangle or arbitrary closed polyline), metres.
fn profile_loop(store: &Store, profile: u32, scale: f64) -> Option<Vec<[f64; 2]>> {
    let rec = store.get(profile)?;
    match rec.ty.as_str() {
        "IFCRECTANGLEPROFILEDEF" => {
            // (ProfileType, Name, Position, XDim, YDim) — centered at Position.
            let f = rec.floats();
            let (xdim, ydim) = (f.get(f.len().wrapping_sub(2))?.abs() * scale, f.last()?.abs() * scale);
            let (cx, cy) = rec.refs().iter().find(|&&r| store.ty_of(r) == Some("IFCAXIS2PLACEMENT2D"))
                .map(|&p| { let o = axis_origin(store, p); (o.0 * scale, o.1 * scale) })
                .unwrap_or((0.0, 0.0));
            let (hx, hy) = (xdim * 0.5, ydim * 0.5);
            Some(vec![[cx - hx, cy - hy], [cx + hx, cy - hy], [cx + hx, cy + hy], [cx - hx, cy + hy]])
        }
        "IFCARBITRARYCLOSEDPROFILEDEF" => {
            // (ProfileType, Name, OuterCurve) → IfcPolyline(points).
            let curve = *rec.refs().last()?;
            polyline_loop(store, curve, scale)
        }
        _ => None,
    }
}

/// Points of an IfcPolyline (list of IfcCartesianPoint), metres.
fn polyline_loop(store: &Store, curve: u32, scale: f64) -> Option<Vec<[f64; 2]>> {
    let rec = store.get(curve)?;
    if rec.ty != "IFCPOLYLINE" {
        return None;
    }
    let pts: Vec<[f64; 2]> = rec.refs().iter().map(|&p| {
        let c = cartesian_point(store, p);
        [c[0] * scale, c[1] * scale]
    }).collect();
    if pts.len() < 3 { None } else { Some(pts) }
}

/// Footprint of a tessellated face set: the XY convex hull of its coordinate
/// list, plus the z-extent (for height). Coordinates come from the referenced
/// IfcCartesianPointList3D (a flat `((x,y,z),...)` list).
fn faceset_footprint(store: &Store, faceset: u32, scale: f64) -> Option<(Vec<[f64; 2]>, f64, f64)> {
    let rec = store.get(faceset)?;
    // Coordinates ref (IfcCartesianPointList3D) — the first entity reference.
    let coords = *rec.refs().first()?;
    let cl = store.get(coords)?;
    let f = cl.floats();
    if f.len() < 9 {
        return None;
    }
    let mut pts2d = Vec::with_capacity(f.len() / 3);
    let mut zlo = f64::INFINITY;
    let mut zhi = f64::NEG_INFINITY;
    for t in f.chunks_exact(3) {
        pts2d.push([t[0] * scale, t[1] * scale]);
        zlo = zlo.min(t[2] * scale);
        zhi = zhi.max(t[2] * scale);
    }
    let hull = convex_hull(&pts2d);
    if hull.len() < 3 {
        None
    } else {
        Some((hull, zlo, zhi))
    }
}

/// 2D convex hull (Andrew's monotone chain), CCW, no repeated endpoint.
fn convex_hull(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut p: Vec<[f64; 2]> = pts.to_vec();
    p.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap().then(a[1].partial_cmp(&b[1]).unwrap()));
    p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9);
    if p.len() < 3 {
        return p;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut hull: Vec<[f64; 2]> = Vec::new();
    for &pt in &p {
        while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], pt) <= 0.0 {
            hull.pop();
        }
        hull.push(pt);
    }
    let lower = hull.len() + 1;
    for &pt in p.iter().rev() {
        while hull.len() >= lower && cross(hull[hull.len() - 2], hull[hull.len() - 1], pt) <= 0.0 {
            hull.pop();
        }
        hull.push(pt);
    }
    hull.pop();
    hull
}

impl IfcModel {
    /// Convenience: every footprint loop across all elements (world XY, metres).
    pub fn footprints(&self) -> Vec<Vec<[f64; 2]>> {
        self.elements.iter().flat_map(|e| e.footprints.clone()).collect()
    }

    /// Tight XY bounding box `[xmin, ymin, xmax, ymax]` of all footprints.
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for loops in self.elements.iter().flat_map(|e| &e.footprints) {
            for p in loops {
                b[0] = b[0].min(p[0]);
                b[1] = b[1].min(p[1]);
                b[2] = b[2].max(p[0]);
                b[3] = b[3].max(p[1]);
            }
        }
        if b[0].is_finite() { Some(b) } else { None }
    }
}

#[allow(dead_code)]
fn _unused(_: GeoError) {}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal IFC4 wall: a 5000×300 mm rectangle extruded 2800 mm, placed at
    // (1000, 2000) mm. Exercises units (mm→m), placement, profile, and extrude.
    const WALL: &str = "\
ISO-10303-21;
DATA;
#1=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
#10=IFCCARTESIANPOINT((0.,0.,0.));
#11=IFCAXIS2PLACEMENT3D(#10,$,$);
#12=IFCCARTESIANPOINT((1000.,2000.,0.));
#13=IFCAXIS2PLACEMENT3D(#12,$,$);
#14=IFCLOCALPLACEMENT($,#13);
#20=IFCCARTESIANPOINT((0.,0.));
#21=IFCAXIS2PLACEMENT2D(#20,$);
#22=IFCRECTANGLEPROFILEDEF(.AREA.,$,#21,5000.,300.);
#23=IFCDIRECTION((0.,0.,1.));
#24=IFCEXTRUDEDAREASOLID(#22,#11,#23,2800.);
#30=IFCSHAPEREPRESENTATION(#99,'Body','SweptSolid',(#24));
#31=IFCPRODUCTDEFINITIONSHAPE($,$,(#30));
#40=IFCWALLSTANDARDCASE('guid',$,'Wall-1',$,$,#14,#31,$);
ENDSEC;
END-ISO-10303-21;";

    #[test]
    fn parses_wall_footprint_with_units_and_placement() {
        let m = parse_ifc(WALL).unwrap();
        assert_eq!(m.length_scale, 0.001, "mm → m");
        assert_eq!(m.elements.len(), 1);
        let e = &m.elements[0];
        assert_eq!(e.ifc_type, "IFCWALLSTANDARDCASE");
        assert_eq!(e.name, "Wall-1");
        assert!((e.height - 2.8).abs() < 1e-9, "2800 mm → 2.8 m, got {}", e.height);
        // Footprint = 5×0.3 m rectangle centered at the placement (1.0, 2.0 m).
        let fp = &e.footprints[0];
        assert_eq!(fp.len(), 4);
        let xs: Vec<f64> = fp.iter().map(|p| p[0]).collect();
        let ys: Vec<f64> = fp.iter().map(|p| p[1]).collect();
        let (xmin, xmax) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
        let (ymin, ymax) = (ys.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MIN, f64::max));
        assert!((xmax - xmin - 5.0).abs() < 1e-9, "width 5 m");
        assert!((ymax - ymin - 0.3).abs() < 1e-9, "depth 0.3 m");
        // Centered at (1.0, 2.0): xmin≈-1.5, xmax≈3.5.
        assert!((xmin - (-1.5)).abs() < 1e-9 && (xmax - 3.5).abs() < 1e-9, "x centered at 1.0");
        assert!((ymin - 1.85).abs() < 1e-9 && (ymax - 2.15).abs() < 1e-9, "y centered at 2.0");
    }

    #[test]
    fn map_conversion_projects_footprint_to_crs() {
        // IfcMapConversion shifts the local footprint into the projected CRS.
        let ifc = "\
ISO-10303-21;
DATA;
#1=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#5=IFCMAPCONVERSION(#90,#91,500000.,4000000.,0.,1.,0.,1.);
#10=IFCCARTESIANPOINT((0.,0.,0.));
#11=IFCAXIS2PLACEMENT3D(#10,$,$);
#12=IFCLOCALPLACEMENT($,#11);
#20=IFCAXIS2PLACEMENT2D(#10,$);
#22=IFCRECTANGLEPROFILEDEF(.AREA.,$,#20,2.,2.);
#23=IFCDIRECTION((0.,0.,1.));
#24=IFCEXTRUDEDAREASOLID(#22,#11,#23,3.);
#30=IFCSHAPEREPRESENTATION(#99,'Body','SweptSolid',(#24));
#31=IFCPRODUCTDEFINITIONSHAPE($,$,(#30));
#40=IFCWALL('guid',$,'W',$,$,#12,#31,$);
ENDSEC;
END-ISO-10303-21;";
        let m = parse_ifc(ifc).unwrap();
        let b = m.bounds().unwrap();
        // 2×2 m centered at origin, shifted by (5e5, 4e6).
        assert!((b[0] - 499999.0).abs() < 1e-6 && (b[2] - 500001.0).abs() < 1e-6, "x bounds {b:?}");
        assert!((b[1] - 3999999.0).abs() < 1e-6 && (b[3] - 4000001.0).abs() < 1e-6, "y bounds {b:?}");
    }

    #[test]
    fn spatial_hierarchy_and_faceted_brep() {
        // A wall as an IfcFacetedBrep, contained in a storey at elevation 3 m.
        let ifc = "\
ISO-10303-21;
DATA;
#1=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#5=IFCCARTESIANPOINT((0.,0.,3.));
#6=IFCAXIS2PLACEMENT3D(#5,$,$);
#7=IFCLOCALPLACEMENT($,#6);
#8=IFCBUILDINGSTOREY('g',$,'Level 1',$,$,#7,$,$,.ELEMENT.,3.0);
#10=IFCCARTESIANPOINT((0.,0.,0.));
#11=IFCAXIS2PLACEMENT3D(#10,$,$);
#12=IFCLOCALPLACEMENT($,#11);
#20=IFCCARTESIANPOINT((0.,0.,0.));
#21=IFCCARTESIANPOINT((4.,0.,0.));
#22=IFCCARTESIANPOINT((4.,2.,0.));
#23=IFCCARTESIANPOINT((0.,2.,0.));
#24=IFCCARTESIANPOINT((0.,0.,3.));
#30=IFCPOLYLOOP((#20,#21,#22,#23));
#31=IFCFACEOUTERBOUND(#30,.T.);
#32=IFCFACE((#31));
#33=IFCPOLYLOOP((#20,#21,#24));
#34=IFCFACEOUTERBOUND(#33,.T.);
#35=IFCFACE((#34));
#36=IFCCLOSEDSHELL((#32,#35));
#37=IFCFACETEDBREP(#36);
#40=IFCSHAPEREPRESENTATION(#99,'Body','Brep',(#37));
#41=IFCPRODUCTDEFINITIONSHAPE($,$,(#40));
#50=IFCWALL('w',$,'Wall-A',$,$,#12,#41,$);
#60=IFCRELCONTAINEDINSPATIALSTRUCTURE('r',$,$,$,(#50),#8);
ENDSEC;
END-ISO-10303-21;";
        let m = parse_ifc(ifc).unwrap();
        assert_eq!(m.elements.len(), 1, "faceted-brep wall extracted");
        let e = &m.elements[0];
        // Footprint hull = 4x2 rectangle; z-extent 0..3 → height 3.
        let b = m.bounds().unwrap();
        assert!((b[2] - 4.0).abs() < 1e-9 && (b[3] - 2.0).abs() < 1e-9, "brep hull bounds {b:?}");
        assert!((e.height - 3.0).abs() < 1e-9, "z-extent → height 3, got {}", e.height);
        // Spatial hierarchy: element attached to "Level 1" at elevation 3 m.
        assert_eq!(e.storey_name.as_deref(), Some("Level 1"));
        assert!((e.storey_elevation - 3.0).abs() < 1e-9, "storey elev {}", e.storey_elevation);
        assert_eq!(m.storeys.len(), 1);
        assert_eq!(m.storeys[0].element_count, 1);
    }

    #[test]
    fn same_named_storeys_not_double_counted() {
        // Two storeys both named "Floor", one wall each → each storey counts 1
        // (regression: counting by name would report 2 for each).
        let ifc = "\
ISO-10303-21;
DATA;
#1=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#5=IFCCARTESIANPOINT((0.,0.,0.));
#6=IFCAXIS2PLACEMENT3D(#5,$,$);
#7=IFCLOCALPLACEMENT($,#6);
#8=IFCBUILDINGSTOREY('a',$,'Floor',$,$,#7,$,$,.ELEMENT.,0.0);
#15=IFCCARTESIANPOINT((0.,0.,3.));
#16=IFCAXIS2PLACEMENT3D(#15,$,$);
#17=IFCLOCALPLACEMENT($,#16);
#18=IFCBUILDINGSTOREY('b',$,'Floor',$,$,#17,$,$,.ELEMENT.,3.0);
#20=IFCCARTESIANPOINT((0.,0.,0.));
#21=IFCAXIS2PLACEMENT3D(#20,$,$);
#22=IFCLOCALPLACEMENT($,#21);
#23=IFCCARTESIANPOINT((0.,0.));
#24=IFCAXIS2PLACEMENT2D(#23,$);
#25=IFCRECTANGLEPROFILEDEF(.AREA.,$,#24,2.,2.);
#26=IFCDIRECTION((0.,0.,1.));
#27=IFCEXTRUDEDAREASOLID(#25,#21,#26,2.);
#28=IFCSHAPEREPRESENTATION(#99,'Body','SweptSolid',(#27));
#29=IFCPRODUCTDEFINITIONSHAPE($,$,(#28));
#30=IFCWALL('w1',$,'W1',$,$,#22,#29,$);
#40=IFCWALL('w2',$,'W2',$,$,#22,#29,$);
#50=IFCRELCONTAINEDINSPATIALSTRUCTURE('r1',$,$,$,(#30),#8);
#51=IFCRELCONTAINEDINSPATIALSTRUCTURE('r2',$,$,$,(#40),#18);
ENDSEC;
END-ISO-10303-21;";
        let m = parse_ifc(ifc).unwrap();
        assert_eq!(m.storeys.len(), 2, "two distinct storeys");
        for s in &m.storeys {
            assert_eq!(s.element_count, 1, "each storey counts only its own element, got {}", s.element_count);
        }
    }

    #[test]
    fn triangulated_faceset_footprint() {
        // A box as a triangulated face set → convex-hull footprint (the XY square).
        let ifc = "\
ISO-10303-21;
DATA;
#1=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#10=IFCCARTESIANPOINT((0.,0.,0.));
#11=IFCAXIS2PLACEMENT3D(#10,$,$);
#12=IFCLOCALPLACEMENT($,#11);
#50=IFCCARTESIANPOINTLIST3D(((0.,0.,0.),(6.,0.,0.),(6.,4.,0.),(0.,4.,0.),(0.,0.,3.),(6.,0.,3.),(6.,4.,3.),(0.,4.,3.)));
#51=IFCTRIANGULATEDFACESET(#50,$,$,((1,2,3),(1,3,4),(5,6,7)),$);
#30=IFCSHAPEREPRESENTATION(#99,'Body','Tessellation',(#51));
#31=IFCPRODUCTDEFINITIONSHAPE($,$,(#30));
#40=IFCBUILDINGELEMENTPROXY('guid',$,'Bldg',$,$,#12,#31,$);
ENDSEC;
END-ISO-10303-21;";
        let m = parse_ifc(ifc).unwrap();
        assert_eq!(m.elements.len(), 1);
        let e = &m.elements[0];
        assert!((e.height - 3.0).abs() < 1e-9, "z-extent → height 3, got {}", e.height);
        let b = m.bounds().unwrap();
        assert!((b[0]).abs() < 1e-9 && (b[2] - 6.0).abs() < 1e-9 && (b[3] - 4.0).abs() < 1e-9, "hull bounds {b:?}");
        assert_eq!(e.footprints[0].len(), 4, "box hull = 4 corners");
    }

    #[test]
    fn arbitrary_polyline_profile() {
        let ifc = "\
ISO-10303-21;
DATA;
#1=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#10=IFCCARTESIANPOINT((0.,0.,0.));
#11=IFCAXIS2PLACEMENT3D(#10,$,$);
#12=IFCLOCALPLACEMENT($,#11);
#20=IFCCARTESIANPOINT((0.,0.));
#21=IFCCARTESIANPOINT((4.,0.));
#22=IFCCARTESIANPOINT((4.,3.));
#23=IFCCARTESIANPOINT((0.,3.));
#24=IFCPOLYLINE((#20,#21,#22,#23));
#25=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#24);
#26=IFCDIRECTION((0.,0.,1.));
#27=IFCEXTRUDEDAREASOLID(#25,#11,#26,5.);
#30=IFCSHAPEREPRESENTATION(#99,'Body','SweptSolid',(#27));
#31=IFCPRODUCTDEFINITIONSHAPE($,$,(#30));
#40=IFCSLAB('guid',$,'Slab',$,$,#12,#31,$);
ENDSEC;
END-ISO-10303-21;";
        let m = parse_ifc(ifc).unwrap();
        assert_eq!(m.length_scale, 1.0);
        assert_eq!(m.elements.len(), 1);
        let e = &m.elements[0];
        assert!((e.height - 5.0).abs() < 1e-9);
        assert_eq!(e.footprints[0].len(), 4);
        let b = m.bounds().unwrap();
        assert!((b[0]).abs() < 1e-9 && (b[2] - 4.0).abs() < 1e-9 && (b[3] - 3.0).abs() < 1e-9, "bounds {b:?}");
    }
}

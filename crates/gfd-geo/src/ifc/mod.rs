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

use std::path::Path;

use crate::{GeoError, GeoResult};

mod parse;
use parse::{Record, Store};

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
}

/// Parsed IFC building model: envelope elements + the length→metre scale used.
#[derive(Debug, Clone)]
pub struct IfcModel {
    pub length_scale: f64,
    pub elements: Vec<IfcElement>,
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

    let mut elements = Vec::new();
    for (&id, rec) in store.iter() {
        if !ENVELOPE.contains(&rec.ty.as_str()) {
            continue;
        }
        if let Some(el) = extract_element(&store, id, rec, length_scale) {
            if !el.footprints.is_empty() {
                elements.push(el);
            }
        }
    }
    elements.sort_by_key(|e| e.id);
    Ok(IfcModel { length_scale, elements })
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
fn extract_element(store: &Store, id: u32, rec: &Record, scale: f64) -> Option<IfcElement> {
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
            if store.ty_of(item) == Some("IFCEXTRUDEDAREASOLID") {
                if let Some((loop2d, h, z0)) = extruded_footprint(store, item, scale) {
                    footprints.push(loop2d.iter().map(|p| [p[0] + ox * scale, p[1] + oy * scale]).collect());
                    height = height.max(h);
                    base_z = base_z.min(oz * scale + z0);
                }
            }
        }
    }
    if footprints.is_empty() {
        return None;
    }
    Some(IfcElement { id, ifc_type: rec.ty.clone(), name, footprints, base_z, height })
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

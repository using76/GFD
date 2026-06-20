"""Live smoke: DEM + IFC building burn-in + flood, via the gfd-server RPC.
Demonstrates water diverting AROUND a building footprint (Phase 5 IFC<->SWE)."""
import json, subprocess, sys

# Flat 15x9 DEM, cellsize 1, origin (0,0) -> domain x[0,15] y[0,9].
asc = "ncols 15\nnrows 9\nxllcorner 0\nyllcorner 0\ncellsize 1\nNODATA_value -9999\n" + \
      "\n".join(" ".join("0" for _ in range(15)) for _ in range(9))

# IFC: a 3x3 m building (extruded rect) centered at (7.5, 4.5) -> footprint [6,9]x[3,6].
ifc = """ISO-10303-21;
DATA;
#1=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#10=IFCCARTESIANPOINT((7.5,4.5,0.));
#11=IFCAXIS2PLACEMENT3D(#10,$,$);
#12=IFCLOCALPLACEMENT($,#11);
#13=IFCCARTESIANPOINT((0.,0.,0.));
#14=IFCAXIS2PLACEMENT3D(#13,$,$);
#20=IFCCARTESIANPOINT((0.,0.));
#21=IFCAXIS2PLACEMENT2D(#20,$);
#22=IFCRECTANGLEPROFILEDEF(.AREA.,$,#21,3.,3.);
#23=IFCDIRECTION((0.,0.,1.));
#24=IFCEXTRUDEDAREASOLID(#22,#14,#23,5.);
#30=IFCSHAPEREPRESENTATION(#99,'Body','SweptSolid',(#24));
#31=IFCPRODUCTDEFINITIONSHAPE($,$,(#30));
#40=IFCBUILDINGELEMENTPROXY('guid',$,'Bldg',$,$,#12,#31,$);
ENDSEC;
END-ISO-10303-21;"""

reqs = [
    {"id": 1, "method": "flood.load_dem", "params": {"asc": asc, "manning_n": 0.0, "order": 2}},
    {"id": 2, "method": "flood.load_ifc", "params": {"ifc": ifc, "wall_height": 100.0}},
    {"id": 3, "method": "flood.seed", "params": {"kind": "dam_break", "axis": "x", "position": 3.0, "depth": 3.0}},
    {"id": 4, "method": "flood.run", "params": {"t_end": 1.5}},
    {"id": 5, "method": "flood.result", "params": {"field": "depth"}},
]
inp = "\n".join(json.dumps(r) for r in reqs) + "\n"
out = subprocess.run([sys.argv[1]], input=inp, capture_output=True, text=True).stdout

res = {}
for line in out.splitlines():
    try:
        m = json.loads(line)
    except Exception:
        continue
    res[m["id"]] = m.get("result", {"ERROR": m.get("error")})

print("load_ifc :", res[2])
print("seed vol :", round(res[3].get("volume", 0), 1))
print("run      :", {k: res[4][k] for k in ("time", "steps", "wet_cells", "peak_depth")})
r = res[5]
nc, nr, vals = r["ncols"], r["nrows"], r["values"]
print(f"\ndepth grid ({nc}x{nr})  —  '#'=building(dry, raised bed)  digit=depth*3  '.'=dry\n")
# z_b high where burned -> mark building cells with '#'
zb = r["z_b"]
for rr in range(nr):
    row = ""
    for c in range(nc):
        i = c + rr * nc
        if zb[i] > 50:      # burned building (raised by 100)
            row += " #"
        elif vals[i] > 1e-3:
            row += f" {min(9, int(vals[i]*3))}"
        else:
            row += " ."
    print(row)

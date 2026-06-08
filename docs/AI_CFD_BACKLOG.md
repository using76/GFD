# AI-driven CFD workflow — status & backlog

The AI workbench (`?ai`) drives the CFD-prep loop through Gmsh's OpenCASCADE kernel
(`crates/gfd-gmsh`, `gmsh.*` RPCs in `src/server.rs`, `gui/src/core/commands/gmsh.ts`,
auto-exposed to the AI). This tracks what works and what's left.

## Done

| Capability | Where |
|---|---|
| AI agent loop (chat → commands) | `gui/src/core/llm/agent.ts`, `ChatPanel.tsx`, `AssistantShell.tsx` |
| Primitives / boolean (cut/fuse) / heal (ShapeFix) | `crates/gfd-gmsh/src/lib.rs`, `gmsh.primitive/boolean/heal` |
| Enclosure + fluid extraction (real B-Rep cut) | `gmsh.enclosure`, `gmsh.extract_fluid` |
| Surface tessellation → viewport | `gmsh.tessellate`, `GmshSceneLayer` (ViewportV2) |
| **Mesh → solver**: gmsh tets → `UnstructuredMesh` installed in `state.mesh` | `volume_to_unstructured` (server.rs); `solve.start` runs on it |
| **BC face tagging**: boundary split into inlet/outlet/wall by `flow_axis` | `classify_boundary_patches` (server.rs); `gmsh.mesh` param |
| API key / provider persistence (live chat) | `ChatPanel.tsx` (localStorage) |
| Camera mouse control, token-bounded tool results, coord-aware prompt | ViewportV2 / agent / ChatPanel |

End-to-end verified over stdio: part → enclosure → fluid → mesh
(**~1187 cells / 924 nodes, patches inlet/outlet/wall**) → `solve.start` accepts it.

## Backlog (priority order)

| # | Feature | Status | Where it'd go | Effort |
|---|---|---|---|---|
| 1 | **Packaging**: bundle `gmsh-*.dll` + `gfd-server.exe` into the app (no electron-builder config exists yet) | missing | `gui/package.json` build block / `gui/electron-builder.yml`; `gui/electron/main.js` binary+DLL resolution | M |
| 2 | **Boundary-layer + local size fields** (only `MeshSizeMax` today) | missing | `crates/gfd-gmsh/src/lib.rs` (mesh fields / BoundaryLayer), `gmsh.mesh` params | M |
| 3 | **STEP/STL import into the OCC model** (`gmshModelOccImportShapes`) — current `cad.import.step` is points-only to the legacy arena | missing | new `gmsh.import` (lib + server + command) | M |
| 4 | **Fillet/chamfer via OCC** (`gmshModelOccFillet/Chamfer`) | missing | lib + server + command | M |
| 5 | **Results viz of the gmsh fluid solve**: verify `results.contour` colors the gmsh mesh in ViewportV2 (cell→node interpolation already exists for structured) | partial | `gui/.../ResultsFieldLayer`, `results.*` | M |
| 6 | **Explicit face tagging UI/RPC** (`gmsh.tag_faces` by id/normal) to override the inlet/outlet/wall heuristic | missing | lib (physical groups) + server + command | M |
| 7 | Delete / select / color individual gmsh shapes (currently one whole-model mesh) | missing | `state.gmsh`, ViewportV2 picking, `gmsh.delete` | M |
| 8 | Export the gmsh volume mesh (.msh/.vtk) | missing | `gmshWrite` + `io.*` | S |
| 9 | Units: `ui.units` is display-only; not passed to Gmsh (`Geometry.OCCTargetUnit`) / solver | partial | lib + solve params | S |
| 10 | Mesh quality metrics returned from `gmsh.mesh` (placeholder values now) | partial | `crates/gfd-gmsh` (gmsh quality) → `gmsh.mesh` result | S |
| 11 | Standalone OCCT (opencascade-rs) for OCC ops outside Gmsh — deferred (Gmsh's OCC covers current needs) | deferred | `crates/gfd-occt` (scaffold exists) | L |

## Update — STEP import, packaging, boundary layers

- **STEP/IGES/STL import** ✅ — `gmsh.import` RPC + ChatPanel "📎 Import CAD" button.
  Dirty CAD is auto-healed (OCC fix small edges/faces, sew, dedup) and **huge-unit
  models are auto-normalized** (scale about bbox center to ~1e3 — fixes 2D mesher
  stalls). Verified on a real 5.7MB ANSYS STEP (imports + tessellates to 41k tris).
  Volume-meshing very dirty parts can still fail on overlapping facets (CAD quality).
- **Live elapsed-time** ✅ on running chat actions; API key/provider persisted.
- **Packaging** ✅ (config) — `gui/package.json` `build` block (`extraResources`
  bundles `gfd-server.exe` + `gmsh-*.dll` into `resources/bin`), `npm run dist`
  (electron-builder). `electron/main.js` resolves the packaged backend path. The
  installer build itself (`npm run dist`) is the deploy step (not run in CI here).
- **Boundary layers** ⚠️ partial — `gmsh.mesh` accepts `bl_first/bl_ratio/bl_thickness`
  and configures a Gmsh BoundaryLayer field with a **graceful fallback**: Gmsh's 3D
  boundary-layer field is limited and often can't grow layers on closed fluid
  volumes, so when it can't, the field is removed and a plain tet mesh is returned
  (response notes `boundary_layer: "requested but not generated …"`). Robust 3D prism
  layers would use gfd-mesh's `prism_layer` extrusion — future work.

## Build notes

Pro backend: `cargo build --release --bin gfd-server --features gmsh` with
`GMSH_LIB_DIR` → Gmsh SDK `lib`. The `gmsh-*.dll` is copied next to
`target/release/gfd-server.exe` (loaded from the exe dir at runtime).

> ⚠️ Gmsh is GPL-2+ — linking `libgmsh` makes a distributed build GPL-encumbered.
> Release-time decision: ship GPL, buy a commercial license, or isolate Gmsh.

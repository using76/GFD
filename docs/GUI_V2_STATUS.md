# GFD GUI v2 (AI-controllable workbench) — Status & Roadmap

The v2 GUI is a ground-up rebuild on a framework-agnostic **command-core**: every
operation is a typed, schema-validated command that both the human UI and an
external AI agent dispatch through one pipeline (resolve → validate → consent →
execute → apply → journal → emit). Opt in with `?v2`; the legacy app stays the
default until parity.

Verification gates: `npx tsc --noEmit` (0 errors), `npm test` (61 passing),
`npx vite build` (succeeds), `cargo build --bin gfd-server` (succeeds).

## Feature status (O = done, △ = partial, X = not yet)

### Core architecture
| Capability | Status | Notes |
|---|:--:|---|
| Headless command-core (registry/dispatcher/journal) | O | `gui/src/core` |
| Typed commands + JSON-Schema params (UI+MCP+validation single source) | O | |
| Undo / redo / replay / audit (one journal) | O | exact inverse patches |
| Consent gating for agent commands | O | read-only/confirm/autonomous |
| Observable serializable AppState + stable ids (id == backend shape_id) | O | |
| Entity addressing by id / name / bbox | O | |
| Entity addressing by ray / screen / nearest / semantic | △ | id/name/bbox done; spatial refs stubbed |

### Backend wiring (real Rust)
| Capability | Status | Notes |
|---|:--:|---|
| Real solver run (solve.start/status/field.get) | O | `realSolver` + `calc.run` |
| Real mesh generation (mesh.generate) | O | prerequisite for solve |
| Setup → solver wiring (boundaries + solver settings) | O | `calc.run` reads setup slice |
| Single combined residual from backend | △ | backend reports 1 scalar, not per-equation |

### Geometry / modeling
| Feature | Status | Notes |
|---|:--:|---|
| Create primitives (box/sphere/cylinder/cone/torus) | O | |
| Transforms (translate/rotate/scale/mirror) | O | functional → new node |
| Boolean union (compound merge) | △ | B-Rep merge only; true CSG is mesh-level |
| Linear array pattern | O | |
| Delete / rename / tessellate | O | |
| 2D sketch (new/line/circle/solve) | O | |
| Parametric edit of feature params | △ | params stored; edit re-run UI pending |
| Import (STL/STEP/...) into tree | X | backend import returns loose mesh, not arena shape |
| Export STL string | O | `io.export_stl` |

### CFD prep / mesh / setup / solve / results
| Feature | Status | Notes |
|---|:--:|---|
| Enclosure (fluid domain box) | O | real CAD box from combined bbox |
| Named selections (geometry→patch map) | O | metadata in `prepare` slice |
| Defeaturing | X | use `repair.check` instead for now |
| Mesh settings panel (size/prism/quality) | X | only `mesh.generate` params exposed |
| Physics models / material / boundaries / solver settings | O | `setup.*` commands |
| Run / stop solver, residual stream | O | events + status |
| Results: field list + stats + load field | O | `results.load_field` |
| Contour / vector / streamline / isosurface render | X | field values fetched; viz layers pending |

### Measure / repair / display
| Feature | Status | Notes |
|---|:--:|---|
| Measure volume / area / center-of-mass / bbox | O | real `cad.measure.*` |
| Measure distance / angle (pick two entities) | X | needs interactive 2-pick flow |
| Repair check / fix / stats | O | real `cad.heal.*` |
| Display render mode / visibility / section plane | O | |

### Pluggable layers (the three "플러그형 구조")
| Capability | Status | Notes |
|---|:--:|---|
| Pluggable LLM provider (Claude/Ollama, tool converters) | O | `llm/` |
| Pluggable physics manifest (GUI) | O | `physics/manifest.ts` + `physics.*` cmds |
| Pluggable physics backend (gfd-expression) | O | `physics.validate_expression/list_builtins/apply_manifest` |
| Expression-defined source terms in the running solver | X | `ExpressionSourceTerm` injection into SIMPLE not wired |

### External AI control
| Capability | Status | Notes |
|---|:--:|---|
| MCP bridge: auto-map registry → tools + meta-tools | O | `mcp/bridge.ts` |
| `get_state` / `list_entities` / `run_command` / `select` / `query_spatial` | O | |
| Headless MCP server (stdio → gfd-server) | O | `mcp-server/`; type-checked, runnable |
| Screenshot vision tool | O | service + renderer capturer + bridge |
| Electron-embedded MCP over WebSocket (live-app screenshots) | X | documented; not wired into main.js |

### Renderer (Phase 3) + adopted 3D libs
| Capability | Status | Notes |
|---|:--:|---|
| R3F ViewportV2 (geometry/camera/picking from AppState) | O | replaces 2,184-line CadScene |
| ViewCube navigation (drei GizmoViewcube) | O | |
| Selection highlight (drei Outlines) | O | |
| Section-plane clipping | O | |
| Accelerated picking (three-mesh-bvh) | O | global raycast patch + boundsTree |
| Transform gizmo drag-to-edit (drei TransformControls) | X | needs node.transform↔mesh binding |
| Measurement overlay (drei Line/Html) | X | |
| Data-driven ribbon + schema forms + feature tree | O | `react/` |
| Retire legacy store / CadScene | X | legacy still default until full parity |

## UI / UX improvements adopted (web research → MIT libraries)
Research surveyed react-three-fiber + **drei**, **three-mesh-bvh**, and CAD
viewers **xeokit** and **gemini-viewer** for the manipulation feature set.

Adopted now (all MIT, no license concerns):
- **drei GizmoViewcube** — SpaceClaim/xeokit-style ViewCube for orientation.
- **drei Outlines** — crisp selection highlight without a postprocessing pass
  (cheaper/safer than EffectComposer Outline for our scene).
- **three-mesh-bvh** — accelerated raycasting for snappy picking on dense meshes.
- **Section-plane clipping** — interactive clip plane (xeokit/gemini parity).
- **preserveDrawingBuffer + projected labels** — enables the AI vision loop
  (annotated screenshots).

Recommended next (researched, queued):
- **drei TransformControls** — move/rotate/scale gizmo, wired to dispatch
  `geometry.translate/rotate/scale` on release (needs the transform↔mesh model).
- **drei `<Line>` + `<Html>`** — distance/angle measurement overlays + 3D labels.
- **camera-controls (yomotsu)** — smoother, CAD-grade camera (fit, dolly-to-cursor).
- **@react-three/postprocessing** Selective Outline/SSAO — richer highlighting.
- **three-bvh-csg** — real mesh boolean (union/diff/intersect) for CSG.
- **xeokit concepts** — x-ray mode, exploded view, navigation plan views,
  annotations, tree-view context menus.

## Remaining work (priority order)
1. Retire legacy store/CadScene once `?v2` reaches parity (mesh-settings,
   contour/vector/streamline viz, distance/angle measure, parametric edit UI).
2. Wire `ExpressionSourceTerm` into the SIMPLE assembly so expression physics
   actually affects the solve (largest backend item).
3. Electron-embedded MCP-over-WebSocket transport for live-app screenshots.
4. Spatial entity refs (ray/screen/nearest/semantic) backed by `cad.measure`.
5. TransformControls drag-to-edit + measurement overlays.

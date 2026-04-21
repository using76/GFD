// JSON-RPC client for the pure-Rust CAD kernel (gfd-server binary).
//
// In the Electron build this proxies through IPC to the spawned gfd-server;
// in browser dev (`npm run dev`) the window object exposes a mock that runs
// an in-memory stub so the UI can be exercised without the Rust backend.

export type RpcRequest = {
  id: number;
  method: string;
  params?: unknown;
};

export type RpcResponse<T = unknown> = {
  id: number;
  result?: T;
  error?: string;
};

type IpcBridge = {
  send: (req: RpcRequest) => Promise<RpcResponse>;
};

declare global {
  interface Window {
    __gfdBridge?: IpcBridge;
  }
}

let nextId = 1;

async function send<T>(method: string, params?: unknown): Promise<T> {
  const bridge = window.__gfdBridge;
  const req: RpcRequest = { id: nextId++, method, params };
  if (!bridge) {
    // Browser dev fallback: simulate a few endpoints so the UI is usable.
    return simulate<T>(method, params);
  }
  const resp = await bridge.send(req);
  if (resp.error) throw new Error(resp.error);
  return resp.result as T;
}

// ---- Simulated responses for browser-only dev ----
let simShapeCounter = 0;
function simulate<T>(method: string, params?: unknown): T {
  switch (method) {
    case 'cad.document.new':
      simShapeCounter = 0;
      return { ok: true } as T;
    case 'cad.document.stats':
      return {
        arena_len: 0, shape_map_len: 0, feature_count: 0, sketch_count: 0, next_shape_id: 0,
      } as T;
    case 'cad.feature.primitive': {
      simShapeCounter += 1;
      const kind = (params as { kind?: string } | undefined)?.kind ?? 'box';
      return {
        shape_id: `shape_${simShapeCounter}`,
        arena_id: simShapeCounter,
        kind,
      } as T;
    }
    case 'cad.tessellate_adaptive':
    case 'cad.tessellate': {
      // Emit a trivial 3-vertex triangle so the viewport has something.
      return {
        positions: [0, 0, 0, 1, 0, 0, 0, 1, 0],
        normals:   [0, 0, 1, 0, 0, 1, 0, 0, 1],
        indices:   [0, 1, 2],
        triangle_count: 1,
      } as T;
    }
    case 'cad.tree.get':
      return { features: [], shapes: [] } as T;
    case 'cad.feature.pad': {
      simShapeCounter += 1;
      return {
        shape_id: `shape_${simShapeCounter}`,
        arena_id: simShapeCounter,
        kind: 'pad',
      } as T;
    }
    case 'cad.feature.revolve': {
      simShapeCounter += 1;
      return {
        shape_id: `shape_${simShapeCounter}`,
        arena_id: simShapeCounter,
        kind: 'revolve',
      } as T;
    }
    case 'cad.feature.pocket': {
      simShapeCounter += 1;
      return {
        shape_id: `shape_${simShapeCounter}`,
        arena_id: simShapeCounter,
        kind: 'pocket',
      } as T;
    }
    case 'cad.feature.chamfer_box':
    case 'cad.feature.fillet_box':
    case 'cad.feature.keycap':
    case 'cad.feature.rounded_top_box':
    case 'cad.feature.mirror':
    case 'cad.feature.translate':
    case 'cad.feature.scale':
    case 'cad.feature.rotate':
    case 'cad.feature.linear_array':
    case 'cad.feature.circular_array': {
      simShapeCounter += 1;
      return {
        shape_id: `shape_${simShapeCounter}`,
        arena_id: simShapeCounter,
        kind: method.split('.').pop() ?? 'unknown',
      } as T;
    }
    case 'cad.boolean.mesh_union':
    case 'cad.boolean.mesh_difference':
    case 'cad.boolean.mesh_intersection':
      return {
        positions: [0, 0, 0, 1, 0, 0, 0, 1, 0],
        normals:   [0, 0, 1, 0, 0, 1, 0, 0, 1],
        indices:   [0, 1, 2],
        triangle_count: 1,
      } as T;
    case 'cad.measure.polygon_area': {
      const pts = ((params as { points?: [number, number][] } | undefined)?.points) ?? [];
      let acc = 0;
      for (let i = 0; i < pts.length; i++) {
        const j = (i + 1) % pts.length;
        acc += pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1];
      }
      return { area: Math.abs(acc) * 0.5 } as T;
    }
    case 'cad.measure.bbox_volume':
      return { volume: 0 } as T;
    case 'cad.measure.surface_area':
      return { area: 0 } as T;
    case 'cad.measure.center_of_mass':
      return { x: 0, y: 0, z: 0 } as T;
    case 'cad.measure.volume':
      return { volume: 0 } as T;
    case 'cad.measure.inertia':
      return { ixx: 0, iyy: 0, izz: 0, ixy: 0, iyz: 0, izx: 0 } as T;
    case 'cad.heal.check_validity':
      return { valid: true, issues: [] } as T;
    case 'cad.heal.fix':
      return { log: ['(sim) no-op'] } as T;
    case 'cad.heal.stats':
      return { vertices: 0, edges: 0, wires: 0, faces: 0, shells: 0, solids: 0 } as T;
    case 'cad.sketch.new':
      return { sketch_idx: 0 } as T;
    case 'cad.sketch.add_point':
      return { point_id: 0 } as T;
    case 'cad.sketch.add_line':
      return { entity_id: 0 } as T;
    case 'cad.sketch.add_constraint':
      return { ok: true, constraint_count: 1 } as T;
    case 'cad.sketch.solve':
      return { residual: 0, points: [] as [number, number][] } as T;
    case 'cad.sketch.dof':
      return { status: 'well', dof: 0, residuals: 0 } as T;
    case 'cad.import.stl':
      return {
        positions: [], normals: [], indices: [], triangle_count: 0,
      } as T;
    default:
      throw new Error(`cadClient (sim): unknown method ${method}`);
  }
}

// ---- Public API ----

export const cadClient = {
  documentNew: () => send<{ ok: boolean }>('cad.document.new'),

  documentStats: () =>
    send<{
      arena_len: number;
      shape_map_len: number;
      feature_count: number;
      sketch_count: number;
      next_shape_id: number;
    }>('cad.document.stats'),

  primitive: (
    kind: 'box' | 'sphere' | 'cylinder' | 'cone' | 'torus',
    params: Record<string, number> = {},
  ) => send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.primitive', { kind, ...params }),

  tessellate: (shape_id: string, u_steps = 32, v_steps = 16) =>
    send<{
      positions: number[];
      normals: number[];
      indices: number[];
      triangle_count: number;
    }>('cad.tessellate', { shape_id, u_steps, v_steps }),

  tessellateAdaptive: (shape_id: string, chord_tolerance = 0.02) =>
    send<{
      positions: number[];
      normals: number[];
      indices: number[];
      triangle_count: number;
    }>('cad.tessellate_adaptive', { shape_id, chord_tolerance }),

  extractEdges: (shape_id: string) =>
    send<{ positions: number[]; line_count: number }>('cad.extract_edges', { shape_id }),

  treeGet: () =>
    send<{ features: unknown[]; shapes: { shape_id: string; arena_id: number }[] }>('cad.tree.get'),

  pad: (points: [number, number][], height: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.pad', { points, height }),

  padProfile: (
    profile: 'ngon' | 'star' | 'rectangle' | 'rounded_rectangle' | 'slot' | 'ellipse' | 'gear' | 'airfoil' | 'i_beam' | 'l_angle' | 'c_channel' | 't_beam' | 'z_section',
    params: Record<string, number>,
  ) =>
    send<{ shape_id: string; arena_id: number; kind: string; polygon_verts: number }>(
      'cad.feature.pad_profile', { profile, ...params },
    ),

  pocketProfile: (
    profile: 'ngon' | 'star' | 'rectangle' | 'rounded_rectangle' | 'slot' | 'ellipse' | 'gear' | 'airfoil' | 'i_beam' | 'l_angle' | 'c_channel' | 't_beam' | 'z_section',
    params: Record<string, number>,
  ) =>
    send<{ shape_id: string; arena_id: number; kind: string; polygon_verts: number }>(
      'cad.feature.pocket_profile', { profile, ...params },
    ),

  revolveProfile: (
    profile: 'ring' | 'cup' | 'frustum' | 'torus' | 'capsule',
    params: Record<string, number>,
  ) =>
    send<{ shape_id: string; arena_id: number; kind: string; profile_verts: number }>(
      'cad.feature.revolve_profile', { profile, ...params },
    ),

  tube: (inner_r: number, outer_r: number, height: number, angular_steps = 24) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.tube', { inner_r, outer_r, height, angular_steps },
    ),

  disc: (radius: number, thickness: number, angular_steps = 24) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.disc', { radius, thickness, angular_steps },
    ),

  tetrahedron: (scale = 0.5) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.tetrahedron', { scale },
    ),

  octahedron: (scale = 0.5) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.octahedron', { scale },
    ),

  icosahedron: (scale = 0.5) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.icosahedron', { scale },
    ),

  dodecahedron: (scale = 0.5) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.dodecahedron', { scale },
    ),

  icosphere: (radius = 0.5, subdivisions = 2) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.icosphere', { radius, subdivisions },
    ),

  honeycomb: (rows = 3, cols = 4, hex_r = 0.3, hex_h = 0.2) =>
    send<{ shape_id: string; arena_id: number; kind: string; cells: number }>(
      'cad.feature.honeycomb', { rows, cols, hex_r, hex_h },
    ),

  spiralStaircase: (
    step_count = 16, radius = 1.0, tread_len = 0.6, tread_w = 0.3,
    step_h = 0.08, angle_per_step_deg = 22.5, rise_per_step = 0.2,
  ) =>
    send<{ shape_id: string; arena_id: number; kind: string; steps: number }>(
      'cad.feature.spiral_staircase',
      { step_count, radius, tread_len, tread_w, step_h, angle_per_step_deg, rise_per_step },
    ),

  stairs: (step_count = 5, step_w = 1.0, step_h = 0.2, step_d = 0.3) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.stairs', { step_count, step_w, step_h, step_d },
    ),

  spiral: (a: number, b: number, turns: number, segments_per_turn = 48) =>
    send<{ points: number[]; point_count: number; a: number; b: number; turns: number }>(
      'cad.profile.spiral', { a, b, turns, segments_per_turn },
    ),

  torusKnot: (p: number, q: number, major_r: number, minor_r: number, segments = 256) =>
    send<{ points: number[]; point_count: number; p: number; q: number; major_r: number; minor_r: number }>(
      'cad.profile.torus_knot', { p, q, major_r, minor_r, segments },
    ),

  helix: (radius: number, pitch: number, turns: number, segments_per_turn = 32) =>
    send<{ points: number[]; point_count: number; length: number; radius: number; pitch: number; turns: number }>(
      'cad.profile.helix', { radius, pitch, turns, segments_per_turn },
    ),

  sketchList: () =>
    send<{
      sketches: { index: number; point_count: number; entity_count: number; constraint_count: number }[];
      count: number;
    }>('cad.sketch.list', {}),

  sketchDelete: (sketch_idx: number) =>
    send<{ deleted: number; remaining: number }>('cad.sketch.delete', { sketch_idx }),

  sketchAddPolyline: (sketch_idx: number, points: number[], closed = false) =>
    send<{ point_ids: number[]; entity_ids: number[]; closed: boolean }>(
      'cad.sketch.add_polyline', { sketch_idx, points, closed },
    ),

  sketchAddProfile: (
    sketch_idx: number,
    profile: 'ngon' | 'star' | 'rectangle' | 'rounded_rectangle' | 'slot' | 'ellipse' | 'gear' | 'airfoil' | 'i_beam' | 'l_angle' | 'c_channel' | 't_beam' | 'z_section',
    params: Record<string, number>,
  ) =>
    send<{ profile: string; point_ids: number[]; entity_ids: number[]; vertex_count: number }>(
      'cad.sketch.add_profile', { sketch_idx, profile, ...params },
    ),

  ping: (payload?: unknown) =>
    send<{ pong: boolean; payload: unknown; unix_ms: number }>(
      'cad.ping', { payload },
    ),

  version: () =>
    send<{
      kernel: string;
      kernel_version: string;
      server_iteration: number;
      features: Record<string, boolean | number | string[]>;
      import_formats: string[];
      export_formats: string[];
      memory_export_formats: string[];
      measure_helpers: number;
      rpc_count_approx: number;
    }>('cad.version', {}),

  listProfileKinds: () =>
    send<{
      pad_kinds: { name: string; params: string[] }[];
      revolve_kinds: { name: string; params: string[] }[];
      primitives: string[];
    }>('cad.profile.list_kinds', {}),

  profileGenerate: (
    profile: 'ngon' | 'star' | 'rectangle' | 'rounded_rectangle' | 'slot' | 'ellipse' | 'gear' | 'airfoil' | 'i_beam' | 'l_angle' | 'c_channel' | 't_beam' | 'z_section',
    params: Record<string, number>,
  ) =>
    send<{ profile: string; points: number[]; vertex_count: number }>(
      'cad.profile.generate', { profile, ...params },
    ),

  polygonSignedArea: (points: [number, number][]) =>
    send<{ signed_area: number; area: number; orientation: 'ccw' | 'cw' | 'degenerate' }>(
      'cad.measure.polygon_signed_area', { points },
    ),

  polygonConvexHull: (points: [number, number][]) =>
    send<{ hull: number[]; vertex_count: number }>(
      'cad.measure.polygon_convex_hull', { points },
    ),

  polygonContainsPoint: (polygon: [number, number][], points: number[]) =>
    send<{ inside: boolean[]; query_count: number }>(
      'cad.measure.polygon_contains_point', { polygon, points },
    ),

  polygonFull: (points: [number, number][]) =>
    send<{
      area: number;
      perimeter: number;
      centroid: [number, number];
      convex: boolean;
      bbox: [[number, number], [number, number]];
      hull_vertex_count: number;
    }>('cad.measure.polygon_full', { points }),

  meshQuality: (shape_id: string, weld = true, tol = 1e-4, u_steps = 32, v_steps = 16) =>
    send<{
      shape_id: string;
      vertex_count: number; triangle_count: number;
      welded: boolean;
      edge_length_stats: [number, number, number, number] | null;
      aspect_ratio_stats: [number, number, number] | null;
      euler_chi: number; genus_estimate: number;
      boundary_edges: number; non_manifold: number;
      is_closed: boolean;
    }>('cad.measure.mesh_quality', { shape_id, weld, tol, u_steps, v_steps }),

  multiShapeSummary: (shape_ids: string[]) =>
    send<{
      summaries: Array<{
        shape_id?: string; arena_id?: number; error?: string;
        surface_area?: number | null; bbox_volume?: number | null;
        divergence_volume?: number | null;
        center_of_mass?: [number, number, number] | null;
        inertia_tensor?: [number, number, number, number, number, number] | null;
        bounding_sphere?: { center: [number, number, number]; radius: number } | null;
        edge_length_range?: [number, number] | null;
        valid?: boolean; issues?: number;
      }>;
      count: number;
    }>('cad.measure.multi_shape_summary', { shape_ids }),

  shapeSummary: (shape_id: string) =>
    send<{
      shape_id: string;
      arena_id: number;
      surface_area: number | null;
      bbox_volume: number | null;
      divergence_volume: number | null;
      center_of_mass: [number, number, number] | null;
      inertia_tensor: [number, number, number, number, number, number] | null;
      bounding_sphere: { center: [number, number, number]; radius: number } | null;
      edge_length_range: [number, number] | null;
      valid: boolean;
      issues: number;
    }>('cad.measure.shape_summary', { shape_id }),

  hausdorff: (shape_a: string, shape_b: string, u_steps = 32, v_steps = 16) =>
    send<{
      distance: number;
      shape_a: string;
      shape_b: string;
      vertex_count_a: number;
      vertex_count_b: number;
    }>('cad.measure.hausdorff', { shape_a, shape_b, u_steps, v_steps }),

  listShapes: () =>
    send<{
      shapes: { shape_id: string; arena_id: number; kind: string }[];
      count: number;
    }>('cad.arena.list_shapes', {}),

  shapeInfo: (shape_id: string) =>
    send<{
      shape_id: string;
      arena_id: number;
      root_kind: string;
      histogram: {
        vertex: number; edge: number; wire: number; face: number;
        shell: number; solid: number; compound: number;
      };
    }>('cad.arena.shape_info', { shape_id }),

  deleteShape: (shape_id: string) =>
    send<{ deleted: string; arena_id: number }>('cad.arena.delete_shape', { shape_id }),

  docSaveJson: (path: string) =>
    send<{ ok: boolean; path: string; shape_count: number }>(
      'cad.document.save_json', { path },
    ),

  docLoadJson: (path: string) =>
    send<{ ok: boolean; path: string; loaded: number }>(
      'cad.document.load_json', { path },
    ),

  docToString: () =>
    send<{ content: string; length: number }>('cad.document.to_string', {}),

  docFromString: (content: string) =>
    send<{ loaded: number }>('cad.document.from_string', { content }),

  arenaStats: () =>
    send<{
      arena_len: number;
      alive: number;
      tombstoned: number;
      registered: number;
      histogram: {
        vertex: number; edge: number; wire: number; face: number;
        shell: number; solid: number; compound: number;
      };
    }>('cad.arena.stats', {}),

  meshSmooth: (positions: number[], indices: number[], iterations = 1, factor = 0.5) =>
    send<{
      positions: number[]; normals: number[]; indices: number[];
      triangle_count: number; vertex_count: number;
      iterations: number; factor: number;
    }>('cad.mesh.smooth', { positions, indices, iterations, factor }),

  meshSubdivide: (positions: number[], indices: number[]) =>
    send<{
      positions: number[]; normals: number[]; indices: number[];
      triangle_count: number; vertex_count: number;
    }>('cad.mesh.subdivide', { positions, indices }),

  meshTransform: (positions: number[], indices: number[], matrix: number[]) =>
    send<{
      positions: number[]; normals: number[]; indices: number[];
      triangle_count: number; vertex_count: number;
    }>('cad.mesh.transform', { positions, indices, matrix }),

  meshComputeNormals: (positions: number[], indices: number[]) =>
    send<{
      positions: number[]; normals: number[]; indices: number[];
      triangle_count: number; vertex_count: number;
    }>('cad.mesh.compute_normals', { positions, indices }),

  meshConcat: (meshes: Array<{ positions: number[]; indices: number[] }>) =>
    send<{
      positions: number[]; normals: number[]; indices: number[];
      triangle_count: number; vertex_count: number; merged_count: number;
    }>('cad.mesh.concat', { meshes }),

  meshReverseWinding: (positions: number[], indices: number[]) =>
    send<{
      positions: number[]; normals: number[]; indices: number[];
      triangle_count: number; vertex_count: number;
    }>('cad.mesh.reverse_winding', { positions, indices }),

  meshWeld: (positions: number[], indices: number[], tol = 1e-4) =>
    send<{
      positions: number[]; normals: number[]; indices: number[];
      triangle_count: number; vertex_count: number;
      welded_removed: number; pruned: number; tol: number;
    }>('cad.mesh.weld', { positions, indices, tol }),

  meshBooleanRaw: (
    a_positions: number[], a_indices: number[],
    b_positions: number[], b_indices: number[],
    op: 'union' | 'difference' | 'intersection',
  ) =>
    send<{
      positions: number[]; normals: number[]; indices: number[];
      triangle_count: number; vertex_count: number;
    }>('cad.mesh.boolean_raw', { a_positions, a_indices, b_positions, b_indices, op }),

  tessellateWelded: (shape_id: string, tol = 1e-4, u_steps = 32, v_steps = 16) =>
    send<{
      positions: number[]; normals: number[]; indices: number[];
      triangle_count: number; vertex_count: number;
      welded_removed: number; pruned: number; tol: number;
    }>('cad.tessellate_welded', { shape_id, tol, u_steps, v_steps }),

  trimeshRaycast: (positions: number[], indices: number[], origins: number[], dirs: number[]) =>
    send<{
      t: number[];
      triangle_index: number[];
      u: number[];
      v: number[];
      ray_count: number;
    }>('cad.measure.trimesh_raycast', { positions, indices, origins, dirs }),

  trimeshSignedDistance: (positions: number[], indices: number[], points: number[]) =>
    send<{
      sdf: number[];
      closest_x: number[]; closest_y: number[]; closest_z: number[];
      query_count: number;
    }>('cad.measure.trimesh_signed_distance', { positions, indices, points }),

  trimeshSummary: (positions: number[], indices: number[]) =>
    send<{
      area: number;
      volume: number;
      bbox: [[number, number, number], [number, number, number]] | null;
      com: [number, number, number] | null;
      inertia: [number, number, number, number, number, number] | null;
      boundary_edges: number;
      non_manifold: number;
      is_closed: boolean;
      euler_chi: number;
      genus_estimate: number;
      vertex_count: number;
      triangle_count: number;
      edge_length_stats: [number, number, number, number] | null;
      aspect_ratio_stats: [number, number, number] | null;
    }>('cad.measure.trimesh_summary', { positions, indices }),

  revolve: (profile: [number, number][], angular_steps = 16, angle_deg?: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.revolve',
      { profile, angular_steps, ...(angle_deg !== undefined ? { angle_deg } : {}) },
    ),

  pocket: (points: [number, number][], depth: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.pocket', { points, depth }),

  chamferBox: (lx: number, ly: number, lz: number, distance: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.chamfer_box', { lx, ly, lz, distance }),

  filletBox: (lx: number, ly: number, lz: number, radius: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.fillet_box', { lx, ly, lz, radius }),

  keycap: (lx: number, ly: number, lz: number, distance: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.keycap', { lx, ly, lz, distance }),

  roundedTopBox: (lx: number, ly: number, lz: number, radius: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.rounded_top_box', { lx, ly, lz, radius }),

  mirror: (shape_id: string, plane: 'xy' | 'yz' | 'xz') =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.mirror', { shape_id, plane }),

  translate: (shape_id: string, tx: number, ty: number, tz: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.translate', { shape_id, tx, ty, tz }),

  scale: (shape_id: string, sx: number, sy: number, sz: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.scale', { shape_id, sx, sy, sz }),

  rotate: (shape_id: string, ax: number, ay: number, az: number, angle_deg: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.rotate', { shape_id, ax, ay, az, angle_deg }),

  linearArray: (shape_id: string, count: number, dx: number, dy: number, dz: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.linear_array', { shape_id, count, dx, dy, dz }),

  circularArray: (shape_id: string, count: number, ax: number, ay: number, az: number, total_deg = 360) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.circular_array', { shape_id, count, ax, ay, az, total_deg }),

  rectangularArray: (
    shape_id: string, count_u: number, count_v: number,
    du_x: number, du_y: number, du_z: number,
    dv_x: number, dv_y: number, dv_z: number,
  ) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.rectangular_array',
      { shape_id, count_u, count_v, du_x, du_y, du_z, dv_x, dv_y, dv_z },
    ),

  offsetPolygon: (points: [number, number][], distance: number) =>
    send<{ points: [number, number][] }>('cad.feature.offset_polygon', { points, distance }),

  offsetPad: (points: [number, number][], offset: number, height: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>(
      'cad.feature.offset_pad', { points, offset, height },
    ),

  wedge: (lx: number, ly: number, lz: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.wedge', { lx, ly, lz }),

  pyramid: (lx: number, ly: number, height: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.pyramid', { lx, ly, height }),

  ngonPrism: (sides: number, radius: number, height: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.ngon_prism', { sides, radius, height }),

  closestPoint: (shape_id: string, x: number, y: number, z: number) =>
    send<{ distance: number }>('cad.measure.closest_point', { shape_id, x, y, z }),

  pointInside: (shape_id: string, x: number, y: number, z: number, u_steps = 16, v_steps = 8) =>
    send<{ inside: boolean }>('cad.measure.point_inside', { shape_id, x, y, z, u_steps, v_steps }),

  signedDistance: (shape_id: string, x: number, y: number, z: number, u_steps = 16, v_steps = 8) =>
    send<{ signed_distance: number; inside: boolean }>(
      'cad.measure.signed_distance', { shape_id, x, y, z, u_steps, v_steps },
    ),

  boundingSphere: (shape_id: string) =>
    send<{ x: number; y: number; z: number; radius: number }>('cad.measure.bounding_sphere', { shape_id }),

  principalAxes: (shape_id: string) =>
    send<{ moments: [number, number, number]; axes: [number, number, number][] }>(
      'cad.measure.principal_axes', { shape_id },
    ),

  edgeLengthRange: (shape_id: string) =>
    send<{ min: number; max: number }>('cad.measure.edge_length_range', { shape_id }),

  filletedCylinder: (radius: number, height: number, fillet: number) =>
    send<{ shape_id: string; arena_id: number; kind: string }>('cad.feature.filleted_cylinder', { radius, height, fillet }),

  meshBoolean: (op: 'union' | 'difference' | 'intersection', a: string, b: string, u_steps = 16, v_steps = 8) =>
    send<{ positions: number[]; normals: number[]; indices: number[]; triangle_count: number }>(
      `cad.boolean.mesh_${op}`,
      { a, b, u_steps, v_steps },
    ),

  importStl: (path: string) =>
    send<{
      positions: number[];
      normals: number[];
      indices: number[];
      triangle_count: number;
    }>('cad.import.stl', { path }),

  importMesh: (kind: 'obj' | 'off' | 'ply' | 'xyz', path: string) =>
    send<{
      positions: number[];
      normals: number[];
      indices: number[];
      triangle_count: number;
      vertex_count: number;
      kind: string;
    }>(`cad.import.${kind}`, { path }),

  exportStl: (shape_id: string, path: string, binary = false, u_steps = 32, v_steps = 16) =>
    send<{ ok: boolean; path: string; triangle_count: number; binary: boolean }>(
      'cad.export.stl', { shape_id, path, binary, u_steps, v_steps },
    ),

  exportObj: (shape_id: string, path: string, u_steps = 32, v_steps = 16) =>
    send<{ ok: boolean; path: string; triangle_count: number }>(
      'cad.export.obj', { shape_id, path, u_steps, v_steps },
    ),

  exportOff: (shape_id: string, path: string, u_steps = 32, v_steps = 16) =>
    send<{ ok: boolean; path: string; triangle_count: number }>(
      'cad.export.off', { shape_id, path, u_steps, v_steps },
    ),

  exportPly: (shape_id: string, path: string, u_steps = 32, v_steps = 16) =>
    send<{ ok: boolean; path: string; triangle_count: number }>(
      'cad.export.ply', { shape_id, path, u_steps, v_steps },
    ),

  exportWrl: (shape_id: string, path: string, u_steps = 32, v_steps = 16) =>
    send<{ ok: boolean; path: string; triangle_count: number }>(
      'cad.export.wrl', { shape_id, path, u_steps, v_steps },
    ),

  exportXyz: (shape_id: string, path: string, u_steps = 32, v_steps = 16) =>
    send<{ ok: boolean; path: string; point_count: number }>(
      'cad.export.xyz', { shape_id, path, u_steps, v_steps },
    ),

  exportVtk: (shape_id: string, path: string, title = 'gfd-cad export', u_steps = 32, v_steps = 16) =>
    send<{ ok: boolean; path: string; triangle_count: number; vertex_count: number }>(
      'cad.export.vtk', { shape_id, path, title, u_steps, v_steps },
    ),

  exportDxf: (shape_id: string, path: string, u_steps = 32, v_steps = 16) =>
    send<{ ok: boolean; path: string; triangle_count: number }>(
      'cad.export.dxf', { shape_id, path, u_steps, v_steps },
    ),

  exportStlString: (shape_id: string, u_steps = 32, v_steps = 16) =>
    send<{ content: string; triangle_count: number; length: number }>(
      'cad.export.stl_string', { shape_id, u_steps, v_steps },
    ),

  exportObjString: (shape_id: string, u_steps = 32, v_steps = 16) =>
    send<{ content: string; triangle_count: number; length: number }>(
      'cad.export.obj_string', { shape_id, u_steps, v_steps },
    ),

  exportPlyString: (shape_id: string, u_steps = 32, v_steps = 16) =>
    send<{ content: string; triangle_count: number; length: number }>(
      'cad.export.ply_string', { shape_id, u_steps, v_steps },
    ),

  exportStepString: (shape_id: string) =>
    send<{ content: string; length: number }>('cad.export.step_string', { shape_id }),

  exportBrepString: (shape_id: string) =>
    send<{ content: string; length: number }>('cad.export.brep_string', { shape_id }),

  exportVtkString: (shape_id: string, u_steps = 32, v_steps = 16) =>
    send<{ content: string; length: number }>('cad.export.vtk_string', { shape_id, u_steps, v_steps }),

  exportWrlString: (shape_id: string, u_steps = 32, v_steps = 16) =>
    send<{ content: string; length: number }>('cad.export.wrl_string', { shape_id, u_steps, v_steps }),

  exportDxfString: (shape_id: string, u_steps = 32, v_steps = 16) =>
    send<{ content: string; length: number }>('cad.export.dxf_string', { shape_id, u_steps, v_steps }),

  exportXyzString: (shape_id: string, u_steps = 32, v_steps = 16) =>
    send<{ content: string; length: number; point_count: number }>(
      'cad.export.xyz_string', { shape_id, u_steps, v_steps },
    ),

  exportBrep: (path: string, shape_id?: string) =>
    send<{ ok: boolean; path: string }>('cad.export.brep', { path, ...(shape_id ? { shape_id } : {}) }),

  importBrep: (path: string) =>
    send<{ shape_id: string | null }>('cad.import.brep', { path }),

  exportStep: (shape_id: string, path: string) =>
    send<{ ok: boolean; path: string }>('cad.export.step', { shape_id, path }),

  importStep: (path: string) =>
    send<{ shape_id: string; arena_id: number }>('cad.import.step', { path }),

  stepSummary: (path: string) =>
    send<{
      cartesian_points: number; vertex_points: number; edge_curves: number;
      edge_loops: number; face_outer_bounds: number; advanced_faces: number;
      closed_shells: number; manifold_solid_breps: number;
      axis2_placements: number; planes: number;
      cylindrical_surfaces: number; spherical_surfaces: number;
    }>('cad.step.summary', { path }),

  polygonArea: (points: [number, number][]) =>
    send<{ area: number }>('cad.measure.polygon_area', { points }),

  bboxVolume: (shape_id: string) =>
    send<{ volume: number }>('cad.measure.bbox_volume', { shape_id }),

  surfaceArea: (shape_id: string) =>
    send<{ area: number }>('cad.measure.surface_area', { shape_id }),

  centerOfMass: (shape_id: string) =>
    send<{ x: number; y: number; z: number }>('cad.measure.center_of_mass', { shape_id }),

  volume: (shape_id: string) =>
    send<{ volume: number }>('cad.measure.volume', { shape_id }),

  inertia: (shape_id: string) =>
    send<{ ixx: number; iyy: number; izz: number; ixy: number; iyz: number; izx: number }>(
      'cad.measure.inertia', { shape_id },
    ),

  sketchNew: () => send<{ sketch_idx: number }>('cad.sketch.new'),

  sketchAddPoint: (sketch_idx: number, x: number, y: number) =>
    send<{ point_id: number }>('cad.sketch.add_point', { sketch_idx, x, y }),

  sketchAddLine: (sketch_idx: number, a: number, b: number) =>
    send<{ entity_id: number }>('cad.sketch.add_line', { sketch_idx, a, b }),

  sketchAddArc: (sketch_idx: number, center: number, start: number, end: number) =>
    send<{ entity_id: number }>('cad.sketch.add_arc', { sketch_idx, center, start, end }),

  sketchAddCircle: (sketch_idx: number, center: number, radius: number) =>
    send<{ entity_id: number }>('cad.sketch.add_circle', { sketch_idx, center, radius }),

  sketchAddConstraint: (sketch_idx: number, kind: string, extra: Record<string, unknown>) =>
    send<{ ok: boolean; constraint_count: number }>('cad.sketch.add_constraint', { sketch_idx, kind, ...extra }),

  sketchSolve: (sketch_idx: number, tolerance = 1e-8, max_iters = 100) =>
    send<{ residual: number; points: [number, number][] }>('cad.sketch.solve', { sketch_idx, tolerance, max_iters }),

  sketchDof: (sketch_idx: number) =>
    send<{ status: 'under' | 'well' | 'over'; dof: number; residuals: number }>('cad.sketch.dof', { sketch_idx }),

  healCheck: (shape_id: string) =>
    send<{ valid: boolean; issues: { arena_id: number; kind: string; detail: string }[] }>('cad.heal.check_validity', { shape_id }),

  healFix: (
    shape_id: string,
    options: {
      tolerance?: number;
      sew?: boolean;
      fix_wires?: boolean;
      remove_small?: boolean;
      remove_duplicate_faces?: boolean;
    } = {},
  ) =>
    send<{ log: string[] }>('cad.heal.fix', {
      shape_id,
      tolerance: options.tolerance ?? 1.0e-7,
      sew: options.sew ?? true,
      fix_wires: options.fix_wires ?? false,
      remove_small: options.remove_small ?? true,
      remove_duplicate_faces: options.remove_duplicate_faces ?? false,
    }),

  healStats: (shape_id: string) =>
    send<{ vertices: number; edges: number; wires: number; faces: number; shells: number; solids: number }>(
      'cad.heal.stats', { shape_id },
    ),
};

export default cadClient;

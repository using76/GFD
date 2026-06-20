/**
 * Flood (2D shallow-water) commands — Phase 9 GUI integration. Each wraps a
 * `flood.*` backend RPC and patches `AppState.flood`, which `FloodLayer` renders
 * as a terrain heightfield colored by water depth. Auto-exposed to the AI/MCP
 * and the ribbon, so a flood can be loaded, seeded, and run from chat, the CLI,
 * or buttons.
 */

import type { JsonObject, JsonValue } from '../types';
import type { CommandDef, CommandContext } from '../command';
import type { CommandRegistry } from '../registry';
import type { PatchOp } from '../patch';

interface FloodResult {
  field: string;
  ncols: number;
  nrows: number;
  cellsize: number;
  origin: [number, number];
  z_b: number[];
  values: number[];
  range: [number, number];
  time: number;
}

/** Fetch a field raster and patch AppState.flood for the renderer. */
async function refreshFlood(ctx: CommandContext, field: string): Promise<PatchOp[]> {
  const r = await ctx.rpc.request<FloodResult>('flood.result', { field });
  const scene = {
    loaded: true,
    ncols: r.ncols, nrows: r.nrows, cellsize: r.cellsize, origin: r.origin,
    zb: r.z_b, values: r.values, field: r.field, range: r.range, time: r.time,
  };
  return [{ op: 'replace', path: ['flood'], value: scene as unknown as JsonValue }];
}

const floodLoadDem: CommandDef<JsonObject, { ncols: number; nrows: number }> = {
  id: 'flood.load_dem',
  category: 'flood',
  group: 'Setup',
  title: 'Load DEM',
  titleKo: 'DEM 불러오기',
  description: 'Load a terrain DEM (ESRI ASCII .asc) by `path`, or inline `asc` text, into a 2D flood scenario. Options: manning_n, cfl, order (1|2), and per-edge BC (xmin/xmax/ymin/ymax = "wall"|"open").',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      path: { type: 'string' }, asc: { type: 'string' },
      manning_n: { type: 'number', minimum: 0 }, cfl: { type: 'number' }, order: { type: 'integer' },
      xmin: { type: 'string' }, xmax: { type: 'string' }, ymin: { type: 'string' }, ymax: { type: 'string' },
    },
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ ncols: number; nrows: number }>('flood.load_dem', params);
    return { ok: true, result: r, statePatch: await refreshFlood(ctx, 'bed') };
  },
};

const floodSeed: CommandDef<JsonObject, { seeded: string; volume: number }> = {
  id: 'flood.seed',
  category: 'flood',
  group: 'Setup',
  title: 'Seed Water',
  titleKo: '초기 수위 설정',
  description: 'Set the initial water. kind="level" (fill below a free-surface `level`), "disk" (pool at x,y radius depth), or "dam_break" (fill one side of axis x|y at `position` to `depth`).',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      kind: { type: 'string', enum: ['level', 'disk', 'dam_break'] },
      level: { type: 'number' }, x: { type: 'number' }, y: { type: 'number' },
      radius: { type: 'number' }, depth: { type: 'number' },
      axis: { type: 'string', enum: ['x', 'y'] }, position: { type: 'number' },
    },
    required: ['kind'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ seeded: string; volume: number }>('flood.seed', params);
    return { ok: true, result: r, statePatch: await refreshFlood(ctx, 'depth') };
  },
};

const floodRun: CommandDef<JsonObject, { time: number; wet_cells: number; peak_depth: number }> = {
  id: 'flood.run',
  category: 'flood',
  group: 'Run',
  title: 'Run Flood',
  titleKo: '홍수 해석 실행',
  description: 'Advance the shallow-water solver by `t_end` seconds (default 1), updating the hazard (max-depth) map. Call repeatedly to animate; fetch field with `field` ("depth"|"max"|"velocity").',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: { t_end: { type: 'number', minimum: 0 }, steps: { type: 'integer' }, field: { type: 'string' } },
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ time: number; wet_cells: number; peak_depth: number }>('flood.run', params);
    const field = (params.field as string) ?? 'depth';
    return { ok: true, result: r, statePatch: await refreshFlood(ctx, field) };
  },
};

const floodReset: CommandDef<JsonObject, { reset: boolean }> = {
  id: 'flood.reset',
  category: 'flood',
  group: 'Setup',
  title: 'Reset Flood',
  titleKo: '홍수 초기화',
  description: 'Clear the active flood scenario.',
  capability: 'mutate-scene',
  paramsSchema: { type: 'object', properties: {} },
  async run(_params, ctx) {
    const r = await ctx.rpc.request<{ reset: boolean }>('flood.reset', {});
    const empty = { loaded: false, ncols: 0, nrows: 0, cellsize: 1, origin: [0, 0], zb: [], values: [], field: 'depth', range: [0, 0], time: 0 };
    return { ok: true, result: r, statePatch: [{ op: 'replace', path: ['flood'], value: empty as unknown as JsonValue }] };
  },
};

const floodLoadIfc: CommandDef<JsonObject, { elements: number; burned_cells: number }> = {
  id: 'flood.load_ifc',
  category: 'flood',
  group: 'Buildings',
  title: 'Load IFC Buildings',
  titleKo: 'IFC 건물 적용',
  description: 'Load building footprints from an IFC file (`path`) or inline `ifc` text and burn them into the flood bed. `method`: "block" (raise terrain, default), "hole" (reflective wall), "roughness" (high Manning n). Each element is raised by its own height unless `wall_height` is given. Requires a DEM-loaded scenario; IfcMapConversion georeferencing aligns the footprints, and the spatial hierarchy (storeys) is parsed.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      path: { type: 'string' }, ifc: { type: 'string' }, wall_height: { type: 'number', minimum: 0 },
      method: { type: 'string', enum: ['block', 'hole', 'roughness'] }, roughness_n: { type: 'number', minimum: 0 },
    },
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ elements: number; burned_cells: number }>('flood.load_ifc', params);
    return { ok: true, result: r, statePatch: await refreshFlood(ctx, 'bed') };
  },
};

const floodBurnBuildings: CommandDef<JsonObject, { buildings: number; burned_cells: number }> = {
  id: 'flood.burn_buildings',
  category: 'flood',
  group: 'Buildings',
  title: 'Burn Footprints',
  titleKo: '건물 풋프린트 적용',
  description: 'Burn explicit building footprints into the flood model: `footprints` is an array of polygons [[[x,y],...],...] in world coords. `method`: "block" (raise z_b by `wall_height`, default), "hole" (reflective internal wall), or "roughness" (set per-cell Manning `roughness_n`). Cells inside are kept dry.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      footprints: { type: 'array' },
      wall_height: { type: 'number', minimum: 0 },
      method: { type: 'string', enum: ['block', 'hole', 'roughness'] },
      roughness_n: { type: 'number', minimum: 0 },
    },
    required: ['footprints'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ buildings: number; burned_cells: number }>('flood.burn_buildings', params);
    return { ok: true, result: r, statePatch: await refreshFlood(ctx, 'bed') };
  },
};

const floodExportRaster: CommandDef<JsonObject, { path: string; ncols: number; nrows: number }> = {
  id: 'flood.export_raster',
  category: 'flood',
  group: 'Run',
  title: 'Export Raster',
  titleKo: '래스터 내보내기',
  description: 'Write a flood field to a georeferenced raster at `path` (loads in QGIS/ArcGIS). `format` = "asc" (ESRI ASCII, default) or "geotiff" (Float32 GeoTIFF). `field` = depth | max | arrival | velocity | bed.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { path: { type: 'string' }, field: { type: 'string' }, format: { type: 'string', enum: ['asc', 'geotiff'] } },
    required: ['path'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ path: string; ncols: number; nrows: number }>('flood.export_raster', params);
    return { ok: true, result: r };
  },
};

const floodZoom3d: CommandDef<JsonObject, { nx: number; ny: number; nz: number; water_volume_3d: number; swe_volume: number }> = {
  id: 'flood.zoom_3d',
  category: 'flood',
  group: 'Run',
  title: 'Zoom 3D (VOF)',
  titleKo: '3D 줌인 (VOF)',
  description: 'One-way coupling: extract a sub-region [xmin,ymin,xmax,ymax] (default whole domain) of the current 2D SWE solution and initialize a 3D free-surface VOF field (water below the surface, depth-averaged velocity). Returns coupling diagnostics; the 3D water volume matches the 2D volume. `nz`, `freeboard_frac` optional.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: {
      xmin: { type: 'number' }, ymin: { type: 'number' }, xmax: { type: 'number' }, ymax: { type: 'number' },
      nz: { type: 'integer' }, freeboard_frac: { type: 'number' },
    },
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ nx: number; ny: number; nz: number; water_volume_3d: number; swe_volume: number }>('flood.zoom_3d', params);
    return { ok: true, result: r };
  },
};

const floodBuild3dMesh: CommandDef<JsonObject, { cells: number; nodes: number }> = {
  id: 'flood.build_3d_mesh',
  category: 'flood',
  group: 'Run',
  title: 'Build 3D Mesh',
  titleKo: '3D 메시 생성',
  description: 'Build a body-fitted cut-cell 3D mesh by SDF-unioning the terrain bed with building prisms (`buildings`: [{footprint:[[x,y]..], base_z, height}]). Region/resolution via xmin/ymin/xmax/ymax/nx/ny/nz. Returns mesh cell + node counts.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: {
      xmin: { type: 'number' }, ymin: { type: 'number' }, xmax: { type: 'number' }, ymax: { type: 'number' },
      nx: { type: 'integer' }, ny: { type: 'integer' }, nz: { type: 'integer' },
      zmin: { type: 'number' }, zmax: { type: 'number' }, buildings: { type: 'array' },
    },
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ cells: number; nodes: number }>('flood.build_3d_mesh', params);
    return { ok: true, result: r };
  },
};

export function registerFloodCommands(registry: CommandRegistry): void {
  registry.register(floodLoadDem);
  registry.register(floodSeed);
  registry.register(floodRun);
  registry.register(floodLoadIfc);
  registry.register(floodBurnBuildings);
  registry.register(floodExportRaster);
  registry.register(floodZoom3d);
  registry.register(floodBuild3dMesh);
  registry.register(floodReset);
}

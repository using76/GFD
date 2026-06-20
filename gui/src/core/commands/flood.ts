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
  description: 'Load building footprints from an IFC file (`path`) or inline `ifc` text and burn them into the flood bed as impermeable high ground (Block method). Each element is raised by its own height unless `wall_height` is given. Requires a DEM-loaded scenario; IfcMapConversion georeferencing aligns the footprints to the DEM.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: { path: { type: 'string' }, ifc: { type: 'string' }, wall_height: { type: 'number', minimum: 0 } },
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
  description: 'Burn explicit building footprints into the flood bed (Block method): `footprints` is an array of polygons [[[x,y],...],...] in world coords; cells inside are raised by `wall_height` (default 10000 m) and kept dry.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      footprints: { type: 'array' },
      wall_height: { type: 'number', minimum: 0 },
    },
    required: ['footprints'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ buildings: number; burned_cells: number }>('flood.burn_buildings', params);
    return { ok: true, result: r, statePatch: await refreshFlood(ctx, 'bed') };
  },
};

export function registerFloodCommands(registry: CommandRegistry): void {
  registry.register(floodLoadDem);
  registry.register(floodSeed);
  registry.register(floodRun);
  registry.register(floodLoadIfc);
  registry.register(floodBurnBuildings);
  registry.register(floodReset);
}

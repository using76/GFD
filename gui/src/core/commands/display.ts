/**
 * Display commands (Phase 4) — render mode, per-shape visibility, section plane.
 * These mutate the display/visibility state the renderer reads. Visibility and
 * render toggles are view-only (not journaled) to keep the undo history about
 * model edits, not viewing.
 */

import type { JsonValue } from '../types';
import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { DisplayState } from '../state';
import type { PatchOp } from '../patch';

export interface SetRenderModeParams {
  mode: 'shaded' | 'wireframe' | 'shaded_edges';
}

export const setRenderMode: CommandDef<SetRenderModeParams, { mode: string }> = {
  id: 'display.set_render_mode',
  category: 'display',
  group: 'Style',
  title: 'Render Mode',
  titleKo: '렌더 모드',
  description: 'Set the viewport render mode: shaded, wireframe, or shaded with edges.',
  capability: 'view-only',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: { mode: { type: 'string', enum: ['shaded', 'wireframe', 'shaded_edges'] } },
    required: ['mode'],
  },
  async run(params) {
    return {
      ok: true,
      result: { mode: params.mode },
      statePatch: [{ op: 'replace', path: ['display', 'renderMode'], value: params.mode }],
    };
  },
};

export interface SetVisibilityParams {
  shape_id: string;
  visible: boolean;
}

export const setVisibility: CommandDef<SetVisibilityParams, { visible: boolean }> = {
  id: 'display.set_visibility',
  category: 'display',
  group: 'Show',
  title: 'Set Visibility',
  titleKo: '표시 설정',
  description: 'Show or hide a shape.',
  capability: 'view-only',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: { shape_id: { type: 'string' }, visible: { type: 'boolean' } },
    required: ['shape_id', 'visible'],
  },
  async run(params, ctx) {
    if (!ctx.getState().doc.geometry.nodes[params.shape_id]) {
      return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${params.shape_id}"` } };
    }
    return {
      ok: true,
      result: { visible: params.visible },
      statePatch: [{ op: 'replace', path: ['doc', 'geometry', 'nodes', params.shape_id, 'visible'], value: params.visible }],
    };
  },
};

export interface SetOpacityParams {
  opacity: number;
}

export const setOpacity: CommandDef<SetOpacityParams, { opacity: number }> = {
  id: 'display.set_opacity',
  category: 'display',
  group: 'Style',
  title: 'Opacity',
  titleKo: '투명도',
  description: 'Set surface opacity 0–1 (1 = opaque, e.g. 0.4 = see-through to inspect interior).',
  capability: 'view-only',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: { opacity: { type: 'number', minimum: 0, maximum: 1 } },
    required: ['opacity'],
  },
  async run(params) {
    const o = Math.max(0, Math.min(1, params.opacity));
    return { ok: true, result: { opacity: o }, statePatch: [{ op: 'replace', path: ['display', 'opacity'], value: o }] };
  },
};

export interface SetLayerParams {
  layer: 'geometry' | 'mesh' | 'results';
  visible: boolean;
}

export const setLayer: CommandDef<SetLayerParams, { layer: string; visible: boolean }> = {
  id: 'display.set_layer',
  category: 'display',
  group: 'Show',
  title: 'Show/Hide Layer',
  titleKo: '레이어 표시',
  description: 'Show or hide a whole render layer: geometry (shapes), mesh, or results.',
  capability: 'view-only',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: {
      layer: { type: 'string', enum: ['geometry', 'mesh', 'results'] },
      visible: { type: 'boolean' },
    },
    required: ['layer', 'visible'],
  },
  async run(params, ctx) {
    const layers = { ...ctx.getState().display.layers, [params.layer]: params.visible };
    return {
      ok: true,
      result: { layer: params.layer, visible: params.visible },
      statePatch: [{ op: 'replace', path: ['display', 'layers'], value: layers as unknown as JsonValue }],
    };
  },
};

export interface SetSectionPlaneParams {
  enabled?: boolean;
  axis?: 'x' | 'y' | 'z' | 'custom';
  offset?: number;
  /** Arbitrary plane normal [nx,ny,nz]; sets axis='custom' when given. */
  normal?: [number, number, number];
}

export const setSectionPlane: CommandDef<SetSectionPlaneParams, DisplayState['sectionPlane']> = {
  id: 'display.set_section_plane',
  category: 'display',
  group: 'Section',
  title: 'Section Plane',
  titleKo: '단면 평면',
  description: 'Cut the model with a clipping plane to see inside. Use axis (x/y/z) + offset for an axis-aligned cut, or normal [nx,ny,nz] + offset for an arbitrary plane.',
  capability: 'view-only',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: {
      enabled: { type: 'boolean' },
      axis: { type: 'string', enum: ['x', 'y', 'z', 'custom'] },
      offset: { type: 'number' },
      normal: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 },
    },
  },
  async run(params, ctx) {
    const cur = ctx.getState().display.sectionPlane;
    const next = {
      enabled: params.enabled ?? cur.enabled,
      axis: params.normal ? 'custom' : (params.axis ?? cur.axis),
      offset: params.offset ?? cur.offset,
      normal: params.normal ?? cur.normal,
    };
    const patch: PatchOp[] = [{ op: 'replace', path: ['display', 'sectionPlane'], value: next as unknown as JsonValue }];
    return { ok: true, result: next, statePatch: patch };
  },
};

export function registerDisplayCommands(registry: CommandRegistry): void {
  registry.register(setRenderMode);
  registry.register(setVisibility);
  registry.register(setOpacity);
  registry.register(setLayer);
  registry.register(setSectionPlane);
}

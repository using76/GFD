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

export interface SetSectionPlaneParams {
  enabled?: boolean;
  axis?: 'x' | 'y' | 'z';
  offset?: number;
}

export const setSectionPlane: CommandDef<SetSectionPlaneParams, DisplayState['sectionPlane']> = {
  id: 'display.set_section_plane',
  category: 'display',
  group: 'Section',
  title: 'Section Plane',
  titleKo: '단면 평면',
  description: 'Enable/configure the section (clipping) plane by axis and offset.',
  capability: 'view-only',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: {
      enabled: { type: 'boolean' },
      axis: { type: 'string', enum: ['x', 'y', 'z'] },
      offset: { type: 'number' },
    },
  },
  async run(params, ctx) {
    const cur = ctx.getState().display.sectionPlane;
    const next = {
      enabled: params.enabled ?? cur.enabled,
      axis: params.axis ?? cur.axis,
      offset: params.offset ?? cur.offset,
    };
    const patch: PatchOp[] = [{ op: 'replace', path: ['display', 'sectionPlane'], value: next as unknown as JsonValue }];
    return { ok: true, result: next, statePatch: patch };
  },
};

export function registerDisplayCommands(registry: CommandRegistry): void {
  registry.register(setRenderMode);
  registry.register(setVisibility);
  registry.register(setSectionPlane);
}

/**
 * View commands — camera control. The actual Three.js camera is driven by the
 * renderer subscribing to AppState.camera (Phase 3); these commands are the
 * single way (human or agent) to move the view.
 */

import type { JsonValue, Vec3 } from '../types';
import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { CameraState } from '../state';
import type { PatchOp } from '../patch';

type NamedView = 'iso' | 'front' | 'back' | 'top' | 'bottom' | 'left' | 'right';

/** Camera positions for named views, looking at the origin at unit distance. */
const NAMED_POSITIONS: Record<NamedView, Vec3> = {
  iso: [5, 5, 5],
  front: [0, 0, 8],
  back: [0, 0, -8],
  top: [0, 8, 0],
  bottom: [0, -8, 0],
  left: [-8, 0, 0],
  right: [8, 0, 0],
};

export interface SetCameraParams {
  named?: NamedView;
  position?: Vec3;
  target?: Vec3;
  fov?: number;
  projection?: 'perspective' | 'orthographic';
}

export const setCamera: CommandDef<SetCameraParams, CameraState> = {
  id: 'view.set_camera',
  category: 'view',
  group: 'Camera',
  title: 'Set Camera',
  titleKo: '카메라 설정',
  description:
    'Position the viewport camera. Use a named view (iso/front/top/...) or explicit position/target/fov/projection.',
  capability: 'view-only',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: {
      named: { type: 'string', enum: ['iso', 'front', 'back', 'top', 'bottom', 'left', 'right'] },
      position: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 },
      target: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 },
      fov: { type: 'number', minimum: 1, maximum: 179 },
      projection: { type: 'string', enum: ['perspective', 'orthographic'] },
    },
  },
  async run(params, ctx) {
    const cam = ctx.getState().camera;
    const position = params.named ? NAMED_POSITIONS[params.named] : params.position ?? cam.position;
    const next: CameraState = {
      position,
      target: params.target ?? (params.named ? [0, 0, 0] : cam.target),
      up: cam.up,
      fov: params.fov ?? cam.fov,
      projection: params.projection ?? cam.projection,
    };
    const patch: PatchOp[] = [{ op: 'replace', path: ['camera'], value: next as unknown as JsonValue }];
    return { ok: true, result: next, statePatch: patch };
  },
};

const cloneJson = (v: unknown): JsonValue => JSON.parse(JSON.stringify(v)) as JsonValue;

/** Reset camera + render mode + section plane + results-viz to the saved defaults. Undoable. */
export const resetView: CommandDef<Record<string, never>, null> = {
  id: 'view.reset',
  category: 'view',
  group: 'Camera',
  title: 'Reset View',
  titleKo: '뷰 리셋',
  description:
    'Reset the camera, render mode, section plane, and results visualization to the saved defaults. Undoable.',
  capability: 'view-only',
  undoable: true,
  paramsSchema: { type: 'object', properties: {} },
  async run(_params, ctx) {
    const d = ctx.getState().viewDefaults;
    const patch: PatchOp[] = [
      { op: 'replace', path: ['camera'], value: cloneJson(d.camera) },
      { op: 'replace', path: ['display'], value: cloneJson(d.display) },
      { op: 'replace', path: ['viz'], value: cloneJson(d.viz) },
    ];
    return { ok: true, result: null, statePatch: patch };
  },
};

/** Snapshot the current camera/display/viz as the defaults that Reset restores. */
export const saveViewDefaults: CommandDef<Record<string, never>, null> = {
  id: 'view.save_defaults',
  category: 'view',
  group: 'Camera',
  title: 'Save View Defaults',
  titleKo: '뷰 기본값 저장',
  description:
    'Save the current camera, render mode, section plane, and results-viz settings as the defaults that Reset restores (persisted across sessions).',
  capability: 'view-only',
  undoable: false,
  paramsSchema: { type: 'object', properties: {} },
  async run(_params, ctx) {
    const s = ctx.getState();
    const value = cloneJson({ camera: s.camera, display: s.display, viz: s.viz });
    return { ok: true, result: null, statePatch: [{ op: 'replace', path: ['viewDefaults'], value }] };
  },
};

export function registerViewCommands(registry: CommandRegistry): void {
  registry.register(setCamera);
  registry.register(resetView);
  registry.register(saveViewDefaults);
}

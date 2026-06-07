import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createMcpBridge } from '../mcp/bridge';
import { createMockRpcClient } from '../transport/rpcClient';

function makeCore() {
  return createCore({ rpc: createMockRpcClient(() => ({})) });
}

describe('view pipeline: reset / save_defaults', () => {
  it('reset restores the camera to defaults, and the reset itself is undoable', async () => {
    const core = makeCore();
    await core.dispatcher.dispatch({ commandId: 'view.set_camera', params: { named: 'top' }, source: 'human' });
    expect(core.store.getState().camera.position).toEqual([0, 8, 0]);

    await core.dispatcher.dispatch({ commandId: 'view.reset', params: {}, source: 'human' });
    expect(core.store.getState().camera.position).toEqual([5, 5, 5]);

    const undone = await core.dispatcher.undo();
    expect(undone).toBe(true);
    expect(core.store.getState().camera.position).toEqual([0, 8, 0]);
  });

  it('save_defaults snapshots the current view so reset returns to it', async () => {
    const core = makeCore();
    await core.dispatcher.dispatch({ commandId: 'view.set_camera', params: { named: 'front' }, source: 'human' });
    await core.dispatcher.dispatch({ commandId: 'view.save_defaults', params: {}, source: 'human' });
    expect(core.store.getState().viewDefaults.camera.position).toEqual([0, 0, 8]);

    await core.dispatcher.dispatch({ commandId: 'view.set_camera', params: { named: 'iso' }, source: 'human' });
    await core.dispatcher.dispatch({ commandId: 'view.reset', params: {}, source: 'human' });
    expect(core.store.getState().camera.position).toEqual([0, 0, 8]);
  });

  it('reset also restores display + viz settings', async () => {
    const core = makeCore();
    await core.dispatcher.dispatch({ commandId: 'display.set_render_mode', params: { mode: 'wireframe' }, source: 'human' });
    await core.dispatcher.dispatch({ commandId: 'results.set_viz', params: { showContour: false }, source: 'human' });
    expect(core.store.getState().display.renderMode).toBe('wireframe');
    expect(core.store.getState().viz.showContour).toBe(false);

    await core.dispatcher.dispatch({ commandId: 'view.reset', params: {}, source: 'human' });
    expect(core.store.getState().display.renderMode).toBe('shaded');
    expect(core.store.getState().viz.showContour).toBe(true);
  });

  it('exposes camera/render/reset + undo/redo to the AI agent (bridge tools)', () => {
    const core = makeCore();
    const tools = createMcpBridge(core).listTools().map((t) => t.name);
    for (const name of ['view__set_camera', 'view__reset', 'view__save_defaults', 'display__set_render_mode', 'results__set_viz', 'undo', 'redo']) {
      expect(tools).toContain(name);
    }
  });
});

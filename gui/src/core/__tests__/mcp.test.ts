import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createMcpBridge } from '../mcp/bridge';
import { createMockRpcClient } from '../transport/rpcClient';
import { ConsentController } from '../consent';
import type { JsonObject } from '../types';

function fakeBackend() {
  let counter = 0;
  return createMockRpcClient((method: string) => {
    if (method === 'cad.feature.primitive') {
      counter += 1;
      return { shape_id: `shape_${counter}`, arena_id: counter, kind: 'box' };
    }
    return {};
  });
}

describe('Phase 5 MCP bridge', () => {
  it('auto-maps commands to tools and includes meta-tools', () => {
    const bridge = createMcpBridge(createCore({ rpc: fakeBackend() }));
    const names = bridge.listTools().map((t) => t.name);
    expect(names).toContain('get_state');
    expect(names).toContain('list_entities');
    expect(names).toContain('run_command');
    expect(names).toContain('geometry__create_primitive'); // dots mangled
    expect(names).toContain('calc__run');
    // Each command tool carries its paramsSchema as the MCP inputSchema.
    const createTool = bridge.listTools().find((t) => t.name === 'geometry__create_primitive');
    expect(createTool?.inputSchema.required).toContain('kind');
  });

  it('drives an agent scenario: create → summarize → select by name → query', async () => {
    const bridge = createMcpBridge(createCore({ rpc: fakeBackend() }));

    const created = await bridge.callTool('geometry__create_primitive', { kind: 'box', name: 'inlet' });
    expect(created.ok).toBe(true);

    const summary = await bridge.callTool('get_state_summary', {});
    expect(summary.ok).toBe(true);
    const s = summary.content as { shapes: Array<{ name: string }> };
    expect(s.shapes.some((sh) => sh.name === 'inlet')).toBe(true);

    const query = await bridge.callTool('query_spatial', { ref: { by: 'name', name: 'inlet' } });
    expect(query.ok).toBe(true);
    expect((query.content as { ids: string[] }).ids).toContain('shape_1');

    const select = await bridge.callTool('select', { refs: [{ by: 'name', name: 'inlet' }] });
    expect(select.ok).toBe(true);
    // run_command escape hatch reaches the same registry.
    const viaRunCommand = await bridge.callTool('run_command', { commandId: 'geometry.rename', params: { shape_id: 'shape_1', name: 'inlet2' } });
    expect(viaRunCommand.ok).toBe(true);
  });

  it('returns an error for unknown tools', async () => {
    const bridge = createMcpBridge(createCore({ rpc: fakeBackend() }));
    const r = await bridge.callTool('does__not__exist', {} as JsonObject);
    expect(r.ok).toBe(false);
  });

  it('honors consent: read-only mode blocks mutating tools but allows reads', async () => {
    const consent = new ConsentController({ mode: 'read-only' });
    const core = createCore({ rpc: fakeBackend(), consentPolicy: { mode: 'read-only' } });
    // override the dispatcher's consent with our controller-equivalent policy
    core.dispatcher.consent.setPolicy(consent.getPolicy());

    const blocked = await createMcpBridge(core).callTool('geometry__create_primitive', { kind: 'box' });
    expect(blocked.ok).toBe(false);

    const allowed = await createMcpBridge(core).callTool('get_state', {});
    expect(allowed.ok).toBe(true);
  });

  it('exposes screenshot as not-yet-available', async () => {
    const r = await createMcpBridge(createCore({ rpc: fakeBackend() })).callTool('screenshot', {});
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/renderer/i);
  });
});

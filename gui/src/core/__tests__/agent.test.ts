import { describe, it, expect } from 'vitest';
import { compactToolResult, runAgentTurn, type AgentEvent } from '../llm/agent';
import type { LlmChunk, LlmProvider } from '../llm/provider';
import type { JsonObject } from '../types';
import type { McpBridge, McpToolResult, McpToolSchema } from '../mcp/bridge';

/** A provider that plays a different scripted turn on each `chat()` call. */
function scriptedProvider(turns: LlmChunk[][]): LlmProvider {
  let call = 0;
  return {
    id: 'scripted',
    capabilities: { tools: true, vision: false, streaming: true },
    async *chat(): AsyncIterable<LlmChunk> {
      const turn = turns[Math.min(call, turns.length - 1)];
      call += 1;
      for (const c of turn) yield c;
    },
  };
}

/** A bridge that records tool calls and returns a canned result. */
function fakeBridge(
  calls: Array<{ name: string; args: JsonObject }>,
  result: McpToolResult = { ok: true, content: { id: 'shape-1' } }
): McpBridge {
  const tools: McpToolSchema[] = [
    { name: 'geometry__create_primitive', description: 'create', inputSchema: { type: 'object', properties: {} } },
  ];
  return {
    listTools: () => tools,
    async callTool(name, args): Promise<McpToolResult> {
      calls.push({ name, args });
      return result;
    },
  };
}

describe('runAgentTurn', () => {
  it('executes a tool the model requests and feeds the result back', async () => {
    const calls: Array<{ name: string; args: JsonObject }> = [];
    const provider = scriptedProvider([
      // Turn 1: ask to create a box, then a follow-up turn is needed.
      [
        { type: 'text', text: 'Creating a box.' },
        { type: 'tool_use', id: 't1', name: 'geometry__create_primitive', input: { kind: 'box' } },
        { type: 'done' },
      ],
      // Turn 2: no tools → final answer.
      [
        { type: 'text', text: 'Done — a box is in the scene.' },
        { type: 'done' },
      ],
    ]);
    const events: AgentEvent[] = [];
    const { history } = await runAgentTurn(
      { provider, bridge: fakeBridge(calls), history: [], userText: 'make a box' },
      (e) => events.push(e)
    );

    expect(calls).toEqual([{ name: 'geometry__create_primitive', args: { kind: 'box' } }]);

    const types = events.map((e) => e.type);
    expect(types).toContain('tool_call');
    expect(types).toContain('tool_result');
    expect(types[types.length - 1]).toBe('turn_done');

    const toolResult = events.find((e) => e.type === 'tool_result');
    expect(toolResult && toolResult.type === 'tool_result' && toolResult.ok).toBe(true);

    // History keeps the user turn, an assistant turn, the folded tool result, and the final answer.
    expect(history[0]).toEqual({ role: 'user', content: 'make a box' });
    expect(history.some((m) => m.role === 'user' && m.content.includes('[tool geometry__create_primitive result]'))).toBe(true);
    expect(history[history.length - 1]).toEqual({ role: 'assistant', content: 'Done — a box is in the scene.' });
  });

  it('finishes immediately when the model returns no tool calls', async () => {
    const calls: Array<{ name: string; args: JsonObject }> = [];
    const provider = scriptedProvider([[{ type: 'text', text: 'hello' }, { type: 'done' }]]);
    const events: AgentEvent[] = [];
    const { history } = await runAgentTurn(
      { provider, bridge: fakeBridge(calls), history: [], userText: 'hi' },
      (e) => events.push(e)
    );
    expect(calls).toHaveLength(0);
    expect(events.map((e) => e.type)).toEqual(['assistant_text', 'turn_done']);
    expect(history[history.length - 1]).toEqual({ role: 'assistant', content: 'hello' });
  });

  it('stops at maxSteps when the model keeps calling tools', async () => {
    const calls: Array<{ name: string; args: JsonObject }> = [];
    // Always asks for a tool → would loop forever without the cap.
    const provider = scriptedProvider([
      [{ type: 'tool_use', id: 't', name: 'geometry__create_primitive', input: {} }, { type: 'done' }],
    ]);
    const events: AgentEvent[] = [];
    await runAgentTurn(
      { provider, bridge: fakeBridge(calls), history: [], userText: 'go', maxSteps: 3 },
      (e) => events.push(e)
    );
    expect(calls).toHaveLength(3);
    const last = events[events.length - 1];
    expect(last.type === 'turn_done' && last.stopReason).toBe('max_steps');
  });

  it('compactToolResult bounds huge results (coordinate buffers, base64, full state)', () => {
    const huge = { positions: Array.from({ length: 5000 }, (_, i) => i), triangle_count: 192 };
    const out = compactToolResult(huge);
    expect(out.length).toBeLessThan(1400);
    expect(out).toContain('triangle_count');
    expect(out).toContain('more of 5000');

    const img = { image: `data:image/png;base64,${'A'.repeat(50000)}`, width: 800 };
    const outImg = compactToolResult(img);
    expect(outImg.length).toBeLessThan(1400);
    expect(outImg).toContain('chars)');
    expect(outImg).toContain('width');
  });

  it('reports tool failures without throwing', async () => {
    const calls: Array<{ name: string; args: JsonObject }> = [];
    const bridge = fakeBridge(calls, { ok: false, content: null, error: 'boom' });
    const provider = scriptedProvider([
      [{ type: 'tool_use', id: 't', name: 'geometry__create_primitive', input: {} }, { type: 'done' }],
      [{ type: 'text', text: 'recovered' }, { type: 'done' }],
    ]);
    const events: AgentEvent[] = [];
    await runAgentTurn({ provider, bridge, history: [], userText: 'go' }, (e) => events.push(e));
    const tr = events.find((e) => e.type === 'tool_result');
    expect(tr && tr.type === 'tool_result' && tr.ok).toBe(false);
  });
});

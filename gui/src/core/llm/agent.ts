/**
 * In-app agent loop — the glue that lets a chat-driven LLM operate the SAME
 * command registry the workbench exposes. The provider streams text + tool_use;
 * each tool_use is executed through the MCP bridge (consent-gated + journaled by
 * the dispatcher); the tool results are fed back so the model can continue, until
 * it finishes a turn with no tool calls (or a safety cap is reached).
 *
 * Framework-agnostic (no React/Three/Electron) so it is unit-testable with the
 * mock provider + a fake bridge. This is what makes the "AI-only" workbench
 * possible: a human types intent, the model drives the commands, and the 3D
 * viewport (subscribed to AppState) shows the result.
 */

import type { JsonObject } from '../types';
import type { McpBridge } from '../mcp/bridge';
import type { LlmMessage, LlmProvider } from './provider';

/**
 * Compress a tool result for folding back into the conversation. Tool results
 * can be enormous — tessellation returns thousands of coordinates, `get_state`
 * the whole document, `screenshot` a base64 PNG — and folding them verbatim
 * blows the model's context window (the "message too large" error). We cap long
 * strings (base64), summarize long arrays (coordinate buffers), and bound the
 * total length, keeping the result informative but small.
 */
export function compactToolResult(value: unknown, maxLen = 1200): string {
  const seen = new WeakSet<object>();
  const replacer = (_key: string, v: unknown): unknown => {
    if (typeof v === 'string' && v.length > 200) return `${v.slice(0, 120)}…(+${v.length - 120} chars)`;
    if (Array.isArray(v) && v.length > 16) return [...v.slice(0, 8), `…(+${v.length - 8} more of ${v.length})`];
    if (v && typeof v === 'object') {
      if (seen.has(v as object)) return '[circular]';
      seen.add(v as object);
    }
    return v;
  };
  let s: string;
  try {
    s = JSON.stringify(value, replacer) ?? String(value);
  } catch {
    s = String(value);
  }
  if (s.length > maxLen) s = `${s.slice(0, maxLen)}…(truncated, ${s.length} chars total)`;
  return s;
}

export type AgentEvent =
  | { type: 'assistant_text'; text: string }
  | { type: 'tool_call'; id: string; name: string; input: JsonObject }
  | { type: 'tool_result'; id: string; name: string; ok: boolean; result: unknown }
  | { type: 'turn_done'; stopReason?: string }
  | { type: 'error'; message: string };

export interface RunAgentTurnArgs {
  provider: LlmProvider;
  bridge: McpBridge;
  /** Prior conversation (user/assistant). The new userText is appended. */
  history: LlmMessage[];
  userText: string;
  system?: string;
  /** Safety cap on tool-use round-trips per user turn (default 8). */
  maxSteps?: number;
  signal?: AbortSignal;
}

export interface RunAgentTurnResult {
  /**
   * The full updated history (the user turn, assistant replies, and folded-in
   * tool results) so the caller can persist it for the next turn.
   */
  history: LlmMessage[];
}

/**
 * Drive one user turn to completion, emitting events as they happen.
 *
 * Tool results are folded back as a single `user` message per round (and an
 * assistant message is always recorded before it) because the shipped
 * Claude/Ollama adapters only forward user/assistant text and require strictly
 * alternating roles. This keeps the loop functional with the thin adapters and
 * the mock; a future adapter that supports structured tool_result content blocks
 * can replace the folding without touching callers.
 */
export async function runAgentTurn(
  args: RunAgentTurnArgs,
  onEvent: (e: AgentEvent) => void
): Promise<RunAgentTurnResult> {
  const { provider, bridge, system, signal } = args;
  const maxSteps = args.maxSteps ?? 8;
  const tools = bridge.listTools();
  const messages: LlmMessage[] = [...args.history, { role: 'user', content: args.userText }];

  for (let step = 0; step < maxSteps; step++) {
    if (signal?.aborted) {
      onEvent({ type: 'error', message: 'aborted' });
      break;
    }

    let assistantText = '';
    const toolCalls: Array<{ id: string; name: string; input: JsonObject }> = [];
    let stopReason: string | undefined;

    try {
      for await (const chunk of provider.chat({ messages, tools, system })) {
        if (chunk.type === 'text') {
          assistantText += chunk.text;
          onEvent({ type: 'assistant_text', text: chunk.text });
        } else if (chunk.type === 'tool_use') {
          toolCalls.push({ id: chunk.id, name: chunk.name, input: chunk.input });
        } else if (chunk.type === 'done') {
          stopReason = chunk.stopReason;
        }
      }
    } catch (err) {
      onEvent({ type: 'error', message: err instanceof Error ? err.message : String(err) });
      break;
    }

    // No tool calls → this is the model's final answer for the turn.
    if (toolCalls.length === 0) {
      if (assistantText) messages.push({ role: 'assistant', content: assistantText });
      onEvent({ type: 'turn_done', stopReason });
      break;
    }

    // Record an assistant turn (keeps user/assistant alternation valid) then
    // execute every requested tool and fold the results into one user message.
    messages.push({ role: 'assistant', content: assistantText || '(running tools)' });

    const resultLines: string[] = [];
    for (const call of toolCalls) {
      onEvent({ type: 'tool_call', id: call.id, name: call.name, input: call.input });
      let ok = false;
      let result: unknown = null;
      try {
        const r = await bridge.callTool(call.name, call.input);
        ok = r.ok;
        result = r.ok ? r.content : (r.error ?? r.content);
      } catch (err) {
        result = { error: err instanceof Error ? err.message : String(err) };
      }
      onEvent({ type: 'tool_result', id: call.id, name: call.name, ok, result });
      resultLines.push(`[tool ${call.name} ${ok ? 'result' : 'error'}] ${compactToolResult(result)}`);
    }
    messages.push({ role: 'user', content: resultLines.join('\n') });

    if (step === maxSteps - 1) {
      onEvent({ type: 'turn_done', stopReason: 'max_steps' });
    }
  }

  return { history: messages };
}

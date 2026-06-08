/**
 * Anthropic Claude adapter. Uses the Messages API over fetch (no SDK dependency).
 * Defaults to the latest Claude models. Network calls are intentionally untested
 * (no key/network in CI); the pure tool-conversion lives in provider.ts.
 */

import type { JsonObject } from '../types';
import {
  toAnthropicTools,
  type LlmChatRequest,
  type LlmChunk,
  type LlmProvider,
} from './provider';

export interface ClaudeOptions {
  apiKey: string;
  model?: string;
  baseUrl?: string;
}

const DEFAULT_MODEL = 'claude-opus-4-8';

export function createClaudeProvider(opts: ClaudeOptions): LlmProvider {
  const model = opts.model ?? DEFAULT_MODEL;
  const baseUrl = opts.baseUrl ?? 'https://api.anthropic.com';

  return {
    id: 'claude',
    capabilities: { tools: true, vision: true, streaming: true },
    async *chat(req: LlmChatRequest): AsyncIterable<LlmChunk> {
      const body: JsonObject = {
        model,
        max_tokens: req.maxTokens ?? 4096,
        messages: req.messages
          .filter((m) => m.role === 'user' || m.role === 'assistant')
          .map((m) => ({ role: m.role, content: m.content })),
      };
      if (req.system) body.system = req.system;
      if (typeof req.temperature === 'number') body.temperature = req.temperature;
      if (req.tools?.length) body.tools = toAnthropicTools(req.tools);

      const resp = await fetch(`${baseUrl}/v1/messages`, {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          'x-api-key': opts.apiKey,
          'anthropic-version': '2023-06-01',
          // Allow calling the Messages API directly from the Electron renderer
          // (browser-origin request) without a CORS preflight failure.
          'anthropic-dangerous-direct-browser-access': 'true',
        },
        body: JSON.stringify(body),
      });
      if (!resp.ok) {
        throw new Error(`Claude API error ${resp.status}: ${await resp.text()}`);
      }

      // Non-streaming for simplicity; emit the assembled blocks as chunks.
      const data = (await resp.json()) as {
        content?: Array<{ type: string; text?: string; id?: string; name?: string; input?: JsonObject }>;
        stop_reason?: string;
      };
      for (const block of data.content ?? []) {
        if (block.type === 'text' && block.text) {
          yield { type: 'text', text: block.text };
        } else if (block.type === 'tool_use' && block.id && block.name) {
          yield { type: 'tool_use', id: block.id, name: block.name, input: block.input ?? {} };
        }
      }
      yield { type: 'done', stopReason: data.stop_reason };
    },
  };
}

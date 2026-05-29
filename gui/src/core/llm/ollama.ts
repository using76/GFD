/**
 * Ollama (local model) adapter. Talks to a local Ollama server's /api/chat over
 * fetch — proves the provider is swappable to a fully local model with no API
 * key. Network calls are untested in CI.
 */

import type { JsonObject } from '../types';
import { toOpenAiTools, type LlmChatRequest, type LlmChunk, type LlmProvider } from './provider';

export interface OllamaOptions {
  model?: string;
  baseUrl?: string;
}

export function createOllamaProvider(opts: OllamaOptions = {}): LlmProvider {
  const model = opts.model ?? 'llama3.1';
  const baseUrl = opts.baseUrl ?? 'http://127.0.0.1:11434';

  return {
    id: 'ollama',
    capabilities: { tools: true, vision: false, streaming: false },
    async *chat(req: LlmChatRequest): AsyncIterable<LlmChunk> {
      const body: JsonObject = {
        model,
        stream: false,
        messages: [
          ...(req.system ? [{ role: 'system', content: req.system }] : []),
          ...req.messages.map((m) => ({ role: m.role, content: m.content })),
        ],
      };
      if (req.tools?.length) body.tools = toOpenAiTools(req.tools);

      const resp = await fetch(`${baseUrl}/api/chat`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      });
      if (!resp.ok) throw new Error(`Ollama error ${resp.status}: ${await resp.text()}`);

      const data = (await resp.json()) as {
        message?: { content?: string; tool_calls?: Array<{ function?: { name?: string; arguments?: JsonObject } }> };
      };
      const msg = data.message;
      if (msg?.content) yield { type: 'text', text: msg.content };
      for (const call of msg?.tool_calls ?? []) {
        if (call.function?.name) {
          yield { type: 'tool_use', id: call.function.name, name: call.function.name, input: call.function.arguments ?? {} };
        }
      }
      yield { type: 'done' };
    },
  };
}

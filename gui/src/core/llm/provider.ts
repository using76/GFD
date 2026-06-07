/**
 * Pluggable LLM provider layer (Phase 8) — thin, because the primary control
 * plane is the EXTERNAL agent via MCP. This exists for optional in-app helpers
 * (e.g. "suggest a boundary condition", "name these entities") and to satisfy
 * the requirement that the AI model / provider API be swappable.
 *
 * Because the agent tools are already MCP schemas, any provider supporting
 * tool-use can drive the SAME command registry. Adapters convert the shared
 * McpToolSchema into each provider's tool format.
 */

import type { JsonObject, JsonValue } from '../types';
import type { McpToolSchema } from '../mcp/bridge';

export type LlmRole = 'system' | 'user' | 'assistant' | 'tool';

export interface LlmMessage {
  role: LlmRole;
  content: string;
  /** For role:'tool' — the id of the tool call this result answers. */
  toolCallId?: string;
}

export type LlmChunk =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; input: JsonObject }
  | { type: 'done'; stopReason?: string };

export interface LlmChatRequest {
  messages: LlmMessage[];
  tools?: McpToolSchema[];
  system?: string;
  maxTokens?: number;
  temperature?: number;
}

export interface LlmCapabilities {
  tools: boolean;
  vision: boolean;
  streaming: boolean;
}

export interface LlmProvider {
  id: string;
  capabilities: LlmCapabilities;
  chat(req: LlmChatRequest): AsyncIterable<LlmChunk>;
}

export interface LlmProviderConfig {
  activeProvider: string;
  model: string;
  baseUrl?: string;
  /** Reference to the secret (env var name / keychain id), never the key itself. */
  apiKeyRef?: string;
}

/** Convert shared MCP tool schemas to Anthropic Messages `tools`. */
export function toAnthropicTools(tools: McpToolSchema[]): JsonValue {
  return tools.map((t) => ({
    name: t.name,
    description: t.description,
    input_schema: t.inputSchema as unknown as JsonValue,
  }));
}

/** Convert shared MCP tool schemas to OpenAI `tools` (function calling). */
export function toOpenAiTools(tools: McpToolSchema[]): JsonValue {
  return tools.map((t) => ({
    type: 'function',
    function: { name: t.name, description: t.description, parameters: t.inputSchema as unknown as JsonValue },
  }));
}

export class LlmProviderRegistry {
  private providers = new Map<string, LlmProvider>();
  private activeId: string | null = null;

  register(provider: LlmProvider): void {
    this.providers.set(provider.id, provider);
    if (this.activeId === null) this.activeId = provider.id;
  }

  get(id: string): LlmProvider | undefined {
    return this.providers.get(id);
  }

  list(): LlmProvider[] {
    return [...this.providers.values()];
  }

  setActive(id: string): void {
    if (!this.providers.has(id)) throw new Error(`Unknown LLM provider "${id}"`);
    this.activeId = id;
  }

  active(): LlmProvider | null {
    return this.activeId ? this.providers.get(this.activeId) ?? null : null;
  }
}

/** A deterministic provider for tests / offline dev — echoes a scripted reply. */
export function createMockProvider(id = 'mock', script: LlmChunk[] = [{ type: 'done' }]): LlmProvider {
  return {
    id,
    capabilities: { tools: true, vision: false, streaming: true },
    async *chat(): AsyncIterable<LlmChunk> {
      for (const chunk of script) yield chunk;
    },
  };
}

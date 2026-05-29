import { describe, it, expect } from 'vitest';
import {
  LlmProviderRegistry,
  createMockProvider,
  toAnthropicTools,
  toOpenAiTools,
  createLlmRegistry,
  type LlmChunk,
} from '../llm';
import type { McpToolSchema } from '../mcp/bridge';

const sampleTool: McpToolSchema = {
  name: 'geometry__create_primitive',
  description: 'Create a primitive.',
  inputSchema: { type: 'object', properties: { kind: { type: 'string' } }, required: ['kind'] },
};

describe('Phase 8 LLM provider layer', () => {
  it('registers providers and tracks the active one', () => {
    const reg = new LlmProviderRegistry();
    reg.register(createMockProvider('a'));
    reg.register(createMockProvider('b'));
    expect(reg.active()?.id).toBe('a'); // first registered is active by default
    reg.setActive('b');
    expect(reg.active()?.id).toBe('b');
    expect(() => reg.setActive('missing')).toThrow();
  });

  it('streams chunks from a provider', async () => {
    const provider = createMockProvider('mock', [
      { type: 'text', text: 'hello' },
      { type: 'tool_use', id: 't1', name: 'geometry__create_primitive', input: { kind: 'box' } },
      { type: 'done' },
    ]);
    const chunks: LlmChunk[] = [];
    for await (const c of provider.chat({ messages: [{ role: 'user', content: 'hi' }] })) chunks.push(c);
    expect(chunks).toHaveLength(3);
    expect(chunks[1]).toMatchObject({ type: 'tool_use', name: 'geometry__create_primitive' });
  });

  it('converts shared MCP tool schemas to Anthropic and OpenAI formats', () => {
    const anthropic = toAnthropicTools([sampleTool]) as Array<{ name: string; input_schema: unknown }>;
    expect(anthropic[0].name).toBe('geometry__create_primitive');
    expect(anthropic[0].input_schema).toEqual(sampleTool.inputSchema);

    const openai = toOpenAiTools([sampleTool]) as Array<{ type: string; function: { name: string; parameters: unknown } }>;
    expect(openai[0].type).toBe('function');
    expect(openai[0].function.parameters).toEqual(sampleTool.inputSchema);
  });

  it('createLlmRegistry falls back to local ollama when no key is given', () => {
    const reg = createLlmRegistry();
    expect(reg.active()?.id).toBe('ollama');
    expect(reg.get('ollama')).toBeDefined();
  });

  it('createLlmRegistry prefers claude when an api key is provided', () => {
    const reg = createLlmRegistry({ claudeApiKey: 'sk-test' });
    expect(reg.active()?.id).toBe('claude');
    expect(reg.list().map((p) => p.id).sort()).toEqual(['claude', 'ollama']);
  });
});

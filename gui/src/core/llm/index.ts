export * from './provider';
export * from './claude';
export * from './ollama';
export * from './agent';

import { LlmProviderRegistry } from './provider';
import { createClaudeProvider } from './claude';
import { createOllamaProvider } from './ollama';

export interface LlmRegistryOptions {
  claudeApiKey?: string;
  claudeModel?: string;
  ollamaModel?: string;
  ollamaBaseUrl?: string;
  /** Which provider to make active (default: 'claude' if a key is given, else 'ollama'). */
  active?: string;
}

/**
 * Build a provider registry with Claude + Ollama registered. Claude is the
 * default when an API key is available; otherwise the local Ollama provider is
 * active so the app works fully offline.
 */
export function createLlmRegistry(options: LlmRegistryOptions = {}): LlmProviderRegistry {
  const registry = new LlmProviderRegistry();
  if (options.claudeApiKey) {
    registry.register(createClaudeProvider({ apiKey: options.claudeApiKey, model: options.claudeModel }));
  }
  registry.register(createOllamaProvider({ model: options.ollamaModel, baseUrl: options.ollamaBaseUrl }));

  const active = options.active ?? (options.claudeApiKey ? 'claude' : 'ollama');
  registry.setActive(active);
  return registry;
}

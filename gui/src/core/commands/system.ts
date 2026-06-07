/**
 * System commands — read-only backend introspection.
 */

import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';

interface VersionResult {
  name?: string;
  version?: string;
  [k: string]: unknown;
}

export const systemVersion: CommandDef<Record<string, never>, VersionResult> = {
  id: 'system.version',
  category: 'system',
  title: 'Backend Version',
  titleKo: '백엔드 버전',
  description: 'Return the gfd-server kernel name, version, and capabilities.',
  capability: 'read',
  paramsSchema: { type: 'object', properties: {}, additionalProperties: false },
  async run(_params, ctx) {
    const result = await ctx.rpc.request<VersionResult>('system.version');
    return { ok: true, result };
  },
};

export function registerSystemCommands(registry: CommandRegistry): void {
  registry.register(systemVersion);
}

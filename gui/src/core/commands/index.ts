/**
 * Command catalogue assembly.
 *
 * `registerCoreCommands` registers the Phase 0/1 command set onto a registry.
 * As later phases author geometry/display/measure/etc. commands, they add their
 * own `registerXxxCommands` here and automatically surface in BOTH the
 * data-driven ribbon and the MCP tool list — no duplicate wiring.
 *
 * Each domain registers via the generic `registry.register<P, R>` (rather than
 * erased `CommandDef[]` arrays) so per-command param/result types are preserved.
 */

import type { CommandRegistry } from '../registry';
import { registerSystemCommands } from './system';
import { registerMeshCommands } from './mesh';
import { registerCalcCommands } from './calc';
import { registerResultsCommands } from './results';

export * from './system';
export * from './mesh';
export * from './calc';
export * from './results';

export function registerCoreCommands(registry: CommandRegistry): void {
  registerSystemCommands(registry);
  registerMeshCommands(registry);
  registerCalcCommands(registry);
  registerResultsCommands(registry);
}

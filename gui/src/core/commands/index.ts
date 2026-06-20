/**
 * Command catalogue assembly.
 *
 * `registerCoreCommands` registers the full command set onto a registry. New
 * domains add their own `registerXxxCommands` here and automatically surface in
 * BOTH the data-driven ribbon and the MCP tool list — no duplicate wiring.
 *
 * Each domain registers via the generic `registry.register<P, R>` (rather than
 * erased `CommandDef[]` arrays) so per-command param/result types are preserved.
 */

import type { CommandRegistry } from '../registry';
import { registerSystemCommands } from './system';
import { registerGeometryCommands } from './geometry';
import { registerSketchCommands } from './sketch';
import { registerSelectionCommands } from './selection';
import { registerViewCommands } from './view';
import { registerDisplayCommands } from './display';
import { registerMeasureCommands } from './measure';
import { registerRepairCommands } from './repair';
import { registerPrepareCommands } from './prepare';
import { registerMeshCommands } from './mesh';
import { registerSetupCommands } from './setup';
import { registerCalcCommands } from './calc';
import { registerResultsCommands } from './results';
import { registerPhysicsCommands } from './physics';
import { registerIoCommands } from './io';
import { registerGmshCommands } from './gmsh';
import { registerFloodCommands } from './flood';

export * from './system';
export * from './geometry';
export * from './sketch';
export * from './selection';
export * from './view';
export * from './display';
export * from './measure';
export * from './repair';
export * from './prepare';
export * from './mesh';
export * from './setup';
export * from './calc';
export * from './results';
export * from './physics';
export * from './io';
export * from './gmsh';

export function registerCoreCommands(registry: CommandRegistry): void {
  registerSystemCommands(registry);
  registerGeometryCommands(registry);
  registerSketchCommands(registry);
  registerSelectionCommands(registry);
  registerViewCommands(registry);
  registerDisplayCommands(registry);
  registerMeasureCommands(registry);
  registerRepairCommands(registry);
  registerPrepareCommands(registry);
  registerMeshCommands(registry);
  registerSetupCommands(registry);
  registerCalcCommands(registry);
  registerResultsCommands(registry);
  registerPhysicsCommands(registry);
  registerIoCommands(registry);
  registerGmshCommands(registry);
  registerFloodCommands(registry);
}

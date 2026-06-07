/**
 * The command registry — the catalogue every other layer reads from.
 *
 * The ribbon/panels are generated from it (data-driven UI), and the MCP server
 * auto-maps it to tools. Registering a new CommandDef makes it appear in BOTH
 * the human UI and the AI tool surface with zero extra wiring.
 */

import type { CommandCategory, JsonObject } from './types';
import type { CommandDef } from './command';

export class CommandRegistry {
  private commands = new Map<string, CommandDef<JsonObject, unknown>>();

  register<P, R>(def: CommandDef<P, R>): void {
    if (this.commands.has(def.id)) {
      throw new Error(`Command "${def.id}" is already registered`);
    }
    // Definitions are stored type-erased; the dispatcher re-narrows on use.
    this.commands.set(def.id, def as unknown as CommandDef<JsonObject, unknown>);
  }

  registerAll(defs: CommandDef<JsonObject, unknown>[]): void {
    for (const def of defs) this.register(def);
  }

  get(id: string): CommandDef<JsonObject, unknown> | undefined {
    return this.commands.get(id);
  }

  has(id: string): boolean {
    return this.commands.has(id);
  }

  list(): CommandDef<JsonObject, unknown>[] {
    return [...this.commands.values()];
  }

  listByCategory(category: CommandCategory): CommandDef<JsonObject, unknown>[] {
    return this.list().filter((c) => c.category === category);
  }

  /** Commands exposed to AI agents as MCP tools (default: all but opt-outs). */
  listAgentExposed(): CommandDef<JsonObject, unknown>[] {
    return this.list().filter((c) => c.exposeToAgent !== false);
  }

  get size(): number {
    return this.commands.size;
  }
}

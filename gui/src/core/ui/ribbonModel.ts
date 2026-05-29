/**
 * Data-driven ribbon model (Phase 4) — derives the SpaceClaim-style ribbon
 * purely from the command registry. A new CommandDef automatically appears as a
 * ribbon button in its category's tab, so the human UI can never drift from the
 * command/MCP surface.
 *
 * Pure and framework-agnostic so it is unit-tested; the React <Ribbon> just maps
 * this model to buttons.
 */

import type { CommandCategory } from '../types';
import type { CommandRegistry } from '../registry';

export interface RibbonCommand {
  id: string;
  title: string;
  titleKo?: string;
  description: string;
  group: string;
  capability: string;
}

export interface RibbonGroup {
  name: string;
  commands: RibbonCommand[];
}

export interface RibbonTab {
  category: CommandCategory;
  label: string;
  groups: RibbonGroup[];
}

/** Tab order + labels (the 9 SpaceClaim tabs + a few internal categories). */
const TAB_ORDER: { category: CommandCategory; label: string }[] = [
  { category: 'geometry', label: 'Design' },
  { category: 'display', label: 'Display' },
  { category: 'measure', label: 'Measure' },
  { category: 'repair', label: 'Repair' },
  { category: 'prepare', label: 'Prepare' },
  { category: 'mesh', label: 'Mesh' },
  { category: 'setup', label: 'Setup' },
  { category: 'physics', label: 'Physics' },
  { category: 'calc', label: 'Calculation' },
  { category: 'results', label: 'Results' },
  { category: 'view', label: 'View' },
  { category: 'selection', label: 'Select' },
  { category: 'system', label: 'System' },
];

export function buildRibbonModel(registry: CommandRegistry): RibbonTab[] {
  const tabs: RibbonTab[] = [];
  for (const { category, label } of TAB_ORDER) {
    const commands = registry.listByCategory(category);
    if (commands.length === 0) continue;

    const groupMap = new Map<string, RibbonCommand[]>();
    for (const cmd of commands) {
      const group = cmd.group ?? 'General';
      const entry: RibbonCommand = {
        id: cmd.id,
        title: cmd.title,
        titleKo: cmd.titleKo,
        description: cmd.description,
        group,
        capability: cmd.capability,
      };
      const list = groupMap.get(group);
      if (list) list.push(entry);
      else groupMap.set(group, [entry]);
    }

    tabs.push({
      category,
      label,
      groups: [...groupMap.entries()].map(([name, cmds]) => ({ name, commands: cmds })),
    });
  }
  return tabs;
}

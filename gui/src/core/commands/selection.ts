/**
 * Selection commands. Selection is view-only (not undoable) but still flows
 * through the dispatcher so the human UI and AI agent share one path.
 */

import type { JsonValue } from '../types';
import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { Selection } from '../state';

export interface SelectionSetParams {
  ids: string[];
  entityType?: 'shape' | 'face' | 'edge' | 'vertex';
}

export const selectionSet: CommandDef<SelectionSetParams, Selection> = {
  id: 'selection.set',
  category: 'selection',
  title: 'Set Selection',
  titleKo: '선택 설정',
  description: 'Replace the current selection with the given entity ids.',
  capability: 'view-only',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: {
      ids: { type: 'array', items: { type: 'string' } },
      entityType: { type: 'string', enum: ['shape', 'face', 'edge', 'vertex'] },
    },
    required: ['ids'],
  },
  async run(params) {
    const selection: Selection = { entityType: params.entityType ?? 'shape', ids: params.ids };
    return {
      ok: true,
      result: selection,
      statePatch: [{ op: 'replace', path: ['selection'], value: selection as unknown as JsonValue }],
    };
  },
};

export function registerSelectionCommands(registry: CommandRegistry): void {
  registry.register(selectionSet);
}

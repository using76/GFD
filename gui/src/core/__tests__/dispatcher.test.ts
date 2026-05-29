import { describe, it, expect } from 'vitest';
import { CommandRegistry } from '../registry';
import { StateStore } from '../state';
import { Dispatcher } from '../dispatcher';
import { ConsentController } from '../consent';
import { createEntityResolver } from '../entity';
import { createMockRpcClient } from '../transport/rpcClient';
import type { CommandDef } from '../command';
import type { JsonObject } from '../types';

function setup(consent?: ConsentController) {
  const registry = new CommandRegistry();
  const store = new StateStore();
  const rpc = createMockRpcClient(() => ({}), { live: false });
  const resolver = createEntityResolver(() => store.getState(), rpc);
  const dispatcher = new Dispatcher({ registry, store, rpc, resolver, consent });
  return { registry, store, dispatcher };
}

const renameTab: CommandDef<{ tab: string }, { tab: string }> = {
  id: 'ui.set_tab',
  category: 'system',
  title: 'Set Tab',
  description: 'Switch the active ribbon tab.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: { tab: { type: 'string' } },
    required: ['tab'],
  },
  async run(params) {
    return {
      ok: true,
      result: { tab: params.tab },
      statePatch: [{ op: 'replace', path: ['ui', 'activeTab'], value: params.tab }],
    };
  },
};

describe('Dispatcher', () => {
  it('runs a command and applies its state patch', async () => {
    const { registry, store, dispatcher } = setup();
    registry.register(renameTab);

    const outcome = await dispatcher.dispatch({
      commandId: 'ui.set_tab',
      params: { tab: 'mesh' },
      source: 'human',
    });

    expect(outcome.ok).toBe(true);
    expect(store.getState().ui.activeTab).toBe('mesh');
    expect(store.getState().doc.revision).toBe(1);
  });

  it('rejects unknown commands', async () => {
    const { dispatcher } = setup();
    const outcome = await dispatcher.dispatch({ commandId: 'nope', params: {}, source: 'human' });
    expect(outcome.ok).toBe(false);
    expect(outcome.error?.code).toBe('UNKNOWN_COMMAND');
  });

  it('rejects invalid params via the schema validator', async () => {
    const { registry, dispatcher } = setup();
    registry.register(renameTab);
    const outcome = await dispatcher.dispatch({
      commandId: 'ui.set_tab',
      params: {} as JsonObject,
      source: 'human',
    });
    expect(outcome.ok).toBe(false);
    expect(outcome.error?.code).toBe('INVALID_PARAMS');
  });

  it('blocks agent mutations in read-only consent mode but allows humans', async () => {
    const consent = new ConsentController({ mode: 'read-only' });
    const { registry, store, dispatcher } = setup(consent);
    registry.register(renameTab);

    const agent = await dispatcher.dispatch({
      commandId: 'ui.set_tab',
      params: { tab: 'setup' },
      source: 'agent',
    });
    expect(agent.ok).toBe(false);
    expect(agent.error?.code).toBe('CONSENT_DENIED');
    expect(store.getState().ui.activeTab).toBe('geometry'); // unchanged

    const human = await dispatcher.dispatch({
      commandId: 'ui.set_tab',
      params: { tab: 'setup' },
      source: 'human',
    });
    expect(human.ok).toBe(true);
    expect(store.getState().ui.activeTab).toBe('setup');
  });

  it('supports undo and redo through the journal', async () => {
    const { registry, store, dispatcher } = setup();
    registry.register(renameTab);

    await dispatcher.dispatch({ commandId: 'ui.set_tab', params: { tab: 'mesh' }, source: 'human' });
    expect(store.getState().ui.activeTab).toBe('mesh');

    expect(await dispatcher.undo()).toBe(true);
    expect(store.getState().ui.activeTab).toBe('geometry');

    expect(await dispatcher.redo()).toBe(true);
    expect(store.getState().ui.activeTab).toBe('mesh');
  });
});

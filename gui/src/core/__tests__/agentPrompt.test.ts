import { describe, it, expect } from 'vitest';
import { createInitialState } from '../state';
import { SYSTEM_PROMPT, sceneContext } from '../../react/agentPrompt';

describe('agent system prompt + scene context', () => {
  it('teaches the AI the full simulation loop and the new commands', () => {
    // The loop is only "driven by the AI" if the prompt names the steps + tools.
    for (const token of [
      'io.import_mesh',
      'io.import_step',
      'mesh.generate',
      'setup.set_model',
      'calc.run',
      'calc.diagnose',
      'auto_refine',
      'SIMULATION LOOP',
    ]) {
      expect(SYSTEM_PROMPT).toContain(token);
    }
  });

  it('omits the diagnosis block when there is no analysis yet', () => {
    const ctx = sceneContext(createInitialState());
    expect(ctx).toContain('Current scene state');
    expect(ctx).not.toContain('Latest diagnosis');
  });

  it('injects the cached diagnosis (summary + non-info issues) into the context', () => {
    const s = createInitialState();
    s.diagnosis = {
      summary: '[심각한 문제] status=finished, issues=2',
      issues: [
        { code: 'DIVERGENCE', severity: 'error', message: '발산 감지' },
        { code: 'NOTE', severity: 'info', message: '참고' },
      ],
    };
    const ctx = sceneContext(s);
    expect(ctx).toContain('Latest diagnosis');
    expect(ctx).toContain('DIVERGENCE');
    // info-level issues are filtered out of the compact prompt context
    expect(ctx).not.toContain('NOTE');
  });
});

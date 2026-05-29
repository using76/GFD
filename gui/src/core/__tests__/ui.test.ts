import { describe, it, expect } from 'vitest';
import { CommandRegistry } from '../registry';
import { registerCoreCommands } from '../commands';
import { buildRibbonModel } from '../ui/ribbonModel';
import { buildFormFields, initialParams } from '../ui/formModel';

describe('Phase 4 data-driven UI models', () => {
  it('derives ribbon tabs/groups from the registry', () => {
    const registry = new CommandRegistry();
    registerCoreCommands(registry);
    const model = buildRibbonModel(registry);

    const geometry = model.find((t) => t.category === 'geometry');
    expect(geometry?.label).toBe('Design');
    // create_primitive lives in the "Create" group
    const createGroup = geometry?.groups.find((g) => g.name === 'Create');
    expect(createGroup?.commands.some((c) => c.id === 'geometry.create_primitive')).toBe(true);

    // Physics + calc tabs exist because commands are registered there.
    expect(model.some((t) => t.category === 'physics')).toBe(true);
    expect(model.some((t) => t.category === 'calc')).toBe(true);
    // Empty categories (no commands) are omitted.
    expect(model.some((t) => t.category === 'repair')).toBe(false);
  });

  it('derives form fields from a command paramsSchema', () => {
    const registry = new CommandRegistry();
    registerCoreCommands(registry);
    const create = registry.get('geometry.create_primitive')!;
    const fields = buildFormFields(create.paramsSchema);

    const kind = fields.find((f) => f.key === 'kind');
    expect(kind?.type).toBe('enum');
    expect(kind?.required).toBe(true);
    expect(kind?.enumValues).toContain('box');

    const lx = fields.find((f) => f.key === 'lx');
    expect(lx?.type).toBe('number');
    expect(lx?.required).toBe(false);
    expect(lx?.min).toBe(0);
  });

  it('detects vec3 fields and builds initial params', () => {
    const registry = new CommandRegistry();
    registerCoreCommands(registry);
    const cam = registry.get('view.set_camera')!;
    const fields = buildFormFields(cam.paramsSchema);
    expect(fields.find((f) => f.key === 'position')?.type).toBe('vec3');

    const create = registry.get('geometry.create_primitive')!;
    const init = initialParams(buildFormFields(create.paramsSchema));
    expect(init.kind).toBe('box'); // first enum value
  });
});

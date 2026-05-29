/**
 * Schema-driven form model (Phase 4) — derives editable form fields from a
 * command's `paramsSchema` (the same schema MCP uses). The React
 * <CommandFormPanel> renders inputs from this model and dispatches the command,
 * so the human form, the MCP tool, and validation all share one definition.
 */

import type { JsonValue } from '../types';
import type { JsonSchema } from '../schema';

export type FormFieldType = 'number' | 'integer' | 'string' | 'boolean' | 'enum' | 'vec3' | 'unsupported';

export interface FormField {
  key: string;
  label: string;
  type: FormFieldType;
  required: boolean;
  description?: string;
  enumValues?: JsonValue[];
  min?: number;
  max?: number;
  default?: JsonValue;
}

function fieldType(schema: JsonSchema): FormFieldType {
  if (schema.enum) return 'enum';
  if (schema.type === 'array' && schema.items?.type === 'number' && schema.minItems === 3 && schema.maxItems === 3) {
    return 'vec3';
  }
  switch (schema.type) {
    case 'number':
      return 'number';
    case 'integer':
      return 'integer';
    case 'string':
      return 'string';
    case 'boolean':
      return 'boolean';
    default:
      return 'unsupported';
  }
}

export function buildFormFields(schema: JsonSchema): FormField[] {
  if (schema.type !== 'object' || !schema.properties) return [];
  const required = new Set(schema.required ?? []);
  return Object.entries(schema.properties).map(([key, propSchema]) => ({
    key,
    label: propSchema.title ?? key,
    type: fieldType(propSchema),
    required: required.has(key),
    description: propSchema.description,
    enumValues: propSchema.enum,
    min: propSchema.minimum,
    max: propSchema.maximum,
    default: propSchema.default,
  }));
}

/** Build the initial params object for a form (defaults / sensible blanks). */
export function initialParams(fields: FormField[]): Record<string, JsonValue> {
  const params: Record<string, JsonValue> = {};
  for (const f of fields) {
    if (f.default !== undefined) {
      params[f.key] = f.default;
    } else if (f.type === 'enum' && f.enumValues?.length) {
      params[f.key] = f.enumValues[0];
    } else if (f.required) {
      params[f.key] = f.type === 'boolean' ? false : f.type === 'vec3' ? [0, 0, 0] : f.type === 'string' || f.type === 'enum' ? '' : 0;
    }
  }
  return params;
}

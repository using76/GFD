/**
 * Minimal JSON-Schema subset and a dependency-free validator.
 *
 * The same `JsonSchema` value drives THREE things from a single definition:
 *   1. the human UI form (CommandFormPanel renders inputs from it),
 *   2. the MCP tool inputSchema exposed to AI agents,
 *   3. parameter validation in the dispatcher pipeline.
 *
 * This keeps the human UI and the AI tool surface from ever drifting apart.
 *
 * The built-in `basicValidator` covers the schema features we actually use
 * (object/array/scalar types, required, enum, numeric bounds). A richer
 * validator (Ajv) can be injected into the Dispatcher in a later phase without
 * touching command definitions.
 */

import type { JsonValue } from './types';

export type JsonSchemaType =
  | 'object'
  | 'array'
  | 'string'
  | 'number'
  | 'integer'
  | 'boolean'
  | 'null';

export interface JsonSchema {
  type?: JsonSchemaType;
  title?: string;
  description?: string;
  /** For type === 'object'. */
  properties?: Record<string, JsonSchema>;
  required?: string[];
  additionalProperties?: boolean;
  /** For type === 'array'. */
  items?: JsonSchema;
  minItems?: number;
  maxItems?: number;
  /** Constrained value set (any scalar). */
  enum?: JsonValue[];
  /** Numeric bounds. */
  minimum?: number;
  maximum?: number;
  /** Default used by the UI form when a value is absent. */
  default?: JsonValue;
}

export interface ValidationError {
  /** JSON-pointer-ish path to the offending value, e.g. "/dimensions/0". */
  path: string;
  message: string;
}

export interface Validator {
  validate(schema: JsonSchema, value: unknown): ValidationError[];
}

function typeOf(value: unknown): JsonSchemaType | 'undefined' {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (Array.isArray(value)) return 'array';
  const t = typeof value;
  if (t === 'number') return Number.isInteger(value) ? 'integer' : 'number';
  if (t === 'boolean') return 'boolean';
  if (t === 'string') return 'string';
  if (t === 'object') return 'object';
  return 'null';
}

function typeMatches(expected: JsonSchemaType, value: unknown): boolean {
  const actual = typeOf(value);
  if (actual === 'undefined') return false;
  // An integer is an acceptable number.
  if (expected === 'number') return actual === 'number' || actual === 'integer';
  return actual === expected;
}

function validateNode(
  schema: JsonSchema,
  value: unknown,
  path: string,
  errors: ValidationError[]
): void {
  if (schema.type !== undefined && !typeMatches(schema.type, value)) {
    errors.push({
      path: path || '/',
      message: `expected type "${schema.type}" but got "${typeOf(value)}"`,
    });
    return; // Type mismatch makes deeper checks meaningless.
  }

  if (schema.enum !== undefined) {
    const ok = schema.enum.some((e) => e === value);
    if (!ok) {
      errors.push({
        path: path || '/',
        message: `value must be one of ${JSON.stringify(schema.enum)}`,
      });
    }
  }

  if (typeof value === 'number') {
    if (schema.minimum !== undefined && value < schema.minimum) {
      errors.push({ path: path || '/', message: `must be >= ${schema.minimum}` });
    }
    if (schema.maximum !== undefined && value > schema.maximum) {
      errors.push({ path: path || '/', message: `must be <= ${schema.maximum}` });
    }
  }

  if (schema.type === 'object' && value && typeof value === 'object' && !Array.isArray(value)) {
    const obj = value as Record<string, unknown>;
    for (const key of schema.required ?? []) {
      if (!(key in obj) || obj[key] === undefined) {
        errors.push({ path: `${path}/${key}`, message: `missing required property "${key}"` });
      }
    }
    if (schema.properties) {
      for (const [key, childSchema] of Object.entries(schema.properties)) {
        if (key in obj && obj[key] !== undefined) {
          validateNode(childSchema, obj[key], `${path}/${key}`, errors);
        }
      }
    }
  }

  if (schema.type === 'array' && Array.isArray(value)) {
    if (schema.minItems !== undefined && value.length < schema.minItems) {
      errors.push({ path: path || '/', message: `must have >= ${schema.minItems} items` });
    }
    if (schema.maxItems !== undefined && value.length > schema.maxItems) {
      errors.push({ path: path || '/', message: `must have <= ${schema.maxItems} items` });
    }
    if (schema.items) {
      value.forEach((item, i) => validateNode(schema.items as JsonSchema, item, `${path}/${i}`, errors));
    }
  }
}

/** A dependency-free validator covering the schema subset above. */
export const basicValidator: Validator = {
  validate(schema, value) {
    const errors: ValidationError[] = [];
    validateNode(schema, value, '', errors);
    return errors;
  },
};

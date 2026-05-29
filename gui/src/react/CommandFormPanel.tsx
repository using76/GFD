/**
 * Schema-driven command form (Phase 4). Renders inputs from a command's
 * paramsSchema (the same schema MCP exposes) and dispatches the command on Run.
 * One definition → human form + AI tool + validation never drift.
 */

import { useMemo, useState } from 'react';
import { buildFormFields, initialParams, type FormField } from '../core';
import type { JsonValue } from '../core';
import { useCore, useDispatch } from './CoreContext';

type Params = Record<string, JsonValue>;

function FieldInput({ field, value, onChange }: { field: FormField; value: JsonValue; onChange: (v: JsonValue) => void }) {
  const common = { width: '100%', padding: '3px 6px', background: '#1a1a1a', color: '#ddd', border: '1px solid #3a3a3a', borderRadius: 3 };
  switch (field.type) {
    case 'enum':
      return (
        <select value={String(value ?? '')} onChange={(e) => onChange(e.target.value)} style={common}>
          {(field.enumValues ?? []).map((opt) => (
            <option key={String(opt)} value={String(opt)}>
              {String(opt)}
            </option>
          ))}
        </select>
      );
    case 'boolean':
      return <input type="checkbox" checked={!!value} onChange={(e) => onChange(e.target.checked)} />;
    case 'number':
    case 'integer':
      return (
        <input
          type="number"
          value={value === undefined || value === null ? '' : Number(value)}
          min={field.min}
          max={field.max}
          onChange={(e) => onChange(e.target.value === '' ? 0 : Number(e.target.value))}
          style={common}
        />
      );
    case 'vec3': {
      const vec = Array.isArray(value) ? (value as number[]) : [0, 0, 0];
      return (
        <div style={{ display: 'flex', gap: 4 }}>
          {[0, 1, 2].map((i) => (
            <input
              key={i}
              type="number"
              value={vec[i] ?? 0}
              onChange={(e) => {
                const next = [...vec];
                next[i] = Number(e.target.value);
                onChange(next as JsonValue);
              }}
              style={{ ...common, width: '33%' }}
            />
          ))}
        </div>
      );
    }
    case 'string':
      return <input type="text" value={String(value ?? '')} onChange={(e) => onChange(e.target.value)} style={common} />;
    default:
      return <span style={{ color: '#888', fontSize: 11 }}>(unsupported)</span>;
  }
}

export function CommandFormPanel({ commandId }: { commandId: string }) {
  const core = useCore();
  const dispatch = useDispatch();
  const def = core.registry.get(commandId);
  const fields = useMemo(() => (def ? buildFormFields(def.paramsSchema) : []), [def]);
  const [params, setParams] = useState<Params>(() => initialParams(fields));
  const [status, setStatus] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  if (!def) return <div style={{ padding: 10, color: '#f66' }}>Unknown command: {commandId}</div>;

  const run = async () => {
    setBusy(true);
    setStatus(null);
    const outcome = await dispatch(def.id, params);
    setBusy(false);
    setStatus(outcome.ok ? { ok: true, message: 'OK' } : { ok: false, message: outcome.error?.message ?? 'failed' });
  };

  return (
    <div style={{ padding: 10, color: '#ddd', fontSize: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 2 }}>{def.titleKo ?? def.title}</div>
      <div style={{ color: '#888', marginBottom: 8 }}>{def.description}</div>
      {fields.length === 0 && <div style={{ color: '#888', marginBottom: 8 }}>No parameters.</div>}
      {fields.map((f) => (
        <div key={f.key} style={{ marginBottom: 6 }}>
          <label style={{ display: 'block', marginBottom: 2 }}>
            {f.label}
            {f.required ? <span style={{ color: '#f66' }}> *</span> : null}
          </label>
          <FieldInput field={f} value={params[f.key]} onChange={(v) => setParams((p) => ({ ...p, [f.key]: v }))} />
        </div>
      ))}
      <button
        onClick={run}
        disabled={busy}
        style={{ marginTop: 8, padding: '5px 14px', background: '#4096ff', color: '#fff', border: 'none', borderRadius: 4, cursor: 'pointer' }}
      >
        {busy ? 'Running…' : 'Run'}
      </button>
      {status && (
        <div style={{ marginTop: 8, color: status.ok ? '#4caf50' : '#f66' }}>{status.message}</div>
      )}
    </div>
  );
}

import React, { useState } from 'react';
import { Button, Space, message, Divider, Tag, Checkbox } from 'antd';
import cadClient from '../../ipc/cadClient';
import { useCadStore } from '../../store/cadStore';

type Issue = { shape_id: string; kind: string; arena_id: number; detail: string };

/**
 * Repair v2 — surfaces validity issues reported by the Rust healer.
 *
 * Iter 10 scope: bulk `heal.check_validity` across every shape, grouped
 * by issue kind. Auto-fix (sew / close-wires / remove-small) arrives with
 * Phase 11 full implementation.
 */
type StatsRow = {
  shape_id: string;
  root: string;
  vertices: number;
  edges: number;
  wires: number;
  faces: number;
  shells: number;
  solids: number;
  compounds: number;
};

const RepairTabV2: React.FC = () => {
  const shapes = useCadStore((s) => s.shapes);
  const [issues, setIssues] = useState<Issue[]>([]);
  const [stats, setStats] = useState<StatsRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [opSew, setOpSew] = useState(true);
  const [opFixWires, setOpFixWires] = useState(false);
  const [opRemoveSmall, setOpRemoveSmall] = useState(true);

  const runCheck = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const out: Issue[] = [];
      const statRows: StatsRow[] = [];
      for (const s of shapes) {
        const [resp, info] = await Promise.all([
          cadClient.healCheck(s.id).catch(() => ({ valid: true, issues: [] })),
          cadClient.shapeInfo(s.id).catch(() => null),
        ]);
        for (const i of resp.issues) {
          out.push({ shape_id: s.id, kind: i.kind, arena_id: i.arena_id, detail: i.detail });
        }
        statRows.push({
          shape_id: s.id,
          root: info?.root_kind ?? 'unknown',
          vertices:  info?.histogram.vertex   ?? 0,
          edges:     info?.histogram.edge     ?? 0,
          wires:     info?.histogram.wire     ?? 0,
          faces:     info?.histogram.face     ?? 0,
          shells:    info?.histogram.shell    ?? 0,
          solids:    info?.histogram.solid    ?? 0,
          compounds: info?.histogram.compound ?? 0,
        });
      }
      setIssues(out);
      setStats(statRows);
      if (out.length === 0) message.success('All shapes valid.');
      else message.warning(`${out.length} issue(s) across ${shapes.length} shape(s).`);
    } catch (e) {
      message.error(`Check failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const kindColor: Record<string, string> = {
    degenerate_edge: 'orange',
    empty_wire: 'red',
    empty_shell: 'red',
    empty_solid: 'red',
    self_reference: 'magenta',
  };

  return (
    <div style={{ padding: 12, color: '#ddd' }}>
      <h4 style={{ color: '#4096ff', marginTop: 0 }}>Repair v2 — Shape healing</h4>
      <Space>
        <Button type="primary" disabled={busy || shapes.length === 0} onClick={runCheck}>
          Check validity
        </Button>
        <Button
          disabled={busy || shapes.length === 0}
          onClick={async () => {
            try {
              const logs: string[] = [];
              for (const s of shapes) {
                const resp = await cadClient.healFix(s.id, {
                  sew: opSew,
                  fix_wires: opFixWires,
                  remove_small: opRemoveSmall,
                });
                logs.push(`${s.id}: ${resp.log.join('; ')}`);
              }
              message.success(logs.join(' / '));
              await runCheck();
            } catch (e) {
              message.error(`Fix failed: ${(e as Error).message}`);
            }
          }}
        >
          Fix all (apply selected)
        </Button>
        <Checkbox checked={opSew} onChange={(e) => setOpSew(e.target.checked)} style={{ color: '#ccd', fontSize: 11 }}>
          Sew vertices
        </Checkbox>
        <Checkbox checked={opFixWires} onChange={(e) => setOpFixWires(e.target.checked)} style={{ color: '#ccd', fontSize: 11 }}>
          Fix wires
        </Checkbox>
        <Checkbox checked={opRemoveSmall} onChange={(e) => setOpRemoveSmall(e.target.checked)} style={{ color: '#ccd', fontSize: 11 }}>
          Remove small edges
        </Checkbox>
        <span style={{ color: '#889', fontSize: 11 }}>
          {shapes.length} shape(s) · {issues.length} issue(s)
        </span>
      </Space>

      <Divider style={{ borderColor: '#303050', margin: '12px 0' }} />

      {stats.length > 0 && (
        <>
          <div style={{ color: '#ccd', fontSize: 12, marginBottom: 4 }}>Shape stats</div>
          <table style={{ width: '100%', fontSize: 11, borderCollapse: 'collapse', marginBottom: 12 }}>
            <thead>
              <tr style={{ color: '#889', textAlign: 'left' }}>
                <th style={{ padding: '2px 6px' }}>Shape</th>
                <th style={{ padding: '2px 6px' }}>Root</th>
                <th style={{ padding: '2px 6px' }}>V</th>
                <th style={{ padding: '2px 6px' }}>E</th>
                <th style={{ padding: '2px 6px' }}>W</th>
                <th style={{ padding: '2px 6px' }}>F</th>
                <th style={{ padding: '2px 6px' }}>Sh</th>
                <th style={{ padding: '2px 6px' }}>So</th>
                <th style={{ padding: '2px 6px' }}>Cp</th>
              </tr>
            </thead>
            <tbody>
              {stats.map((r) => (
                <tr key={r.shape_id} style={{ borderTop: '1px solid #252540' }}>
                  <td style={{ padding: '2px 6px', color: '#aab' }}>{r.shape_id}</td>
                  <td style={{ padding: '2px 6px', color: '#6bf', fontSize: 10 }}>{r.root}</td>
                  <td style={{ padding: '2px 6px', fontFamily: 'monospace' }}>{r.vertices}</td>
                  <td style={{ padding: '2px 6px', fontFamily: 'monospace' }}>{r.edges}</td>
                  <td style={{ padding: '2px 6px', fontFamily: 'monospace' }}>{r.wires}</td>
                  <td style={{ padding: '2px 6px', fontFamily: 'monospace' }}>{r.faces}</td>
                  <td style={{ padding: '2px 6px', fontFamily: 'monospace' }}>{r.shells}</td>
                  <td style={{ padding: '2px 6px', fontFamily: 'monospace' }}>{r.solids}</td>
                  <td style={{ padding: '2px 6px', fontFamily: 'monospace' }}>{r.compounds}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {issues.length === 0 ? (
        <div style={{ color: '#667', fontSize: 12 }}>
          No issues detected. Click "Check validity" to run a fresh pass.
        </div>
      ) : (
        <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
          <thead>
            <tr style={{ color: '#889', textAlign: 'left' }}>
              <th style={{ padding: '4px 8px' }}>Shape</th>
              <th style={{ padding: '4px 8px' }}>Arena id</th>
              <th style={{ padding: '4px 8px' }}>Kind</th>
              <th style={{ padding: '4px 8px' }}>Detail</th>
            </tr>
          </thead>
          <tbody>
            {issues.map((i, idx) => (
              <tr key={`${i.shape_id}-${idx}`} style={{ borderTop: '1px solid #303050' }}>
                <td style={{ padding: '4px 8px', color: '#aab' }}>{i.shape_id}</td>
                <td style={{ padding: '4px 8px', fontFamily: 'monospace' }}>{i.arena_id}</td>
                <td style={{ padding: '4px 8px' }}>
                  <Tag color={kindColor[i.kind] ?? 'default'}>{i.kind}</Tag>
                </td>
                <td style={{ padding: '4px 8px', color: '#889', fontSize: 11 }}>{i.detail}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
};

export default RepairTabV2;

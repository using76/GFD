import React, { useState } from 'react';
import { Button, Tooltip, message } from 'antd';
import {
  EyeOutlined, EyeInvisibleOutlined, DeleteOutlined, ExportOutlined,
  BorderOutlined, RadiusSettingOutlined, ColumnHeightOutlined,
  AimOutlined, RetweetOutlined, ExpandOutlined, QuestionCircleOutlined,
  CheckCircleOutlined, WarningOutlined,
} from '@ant-design/icons';
import { useCadStore } from '../../store/cadStore';
import cadClient from '../../ipc/cadClient';

const KIND_ICON: Record<string, React.ReactNode> = {
  box:      <BorderOutlined style={{ color: '#4096ff' }} />,
  sphere:   <RadiusSettingOutlined style={{ color: '#52c41a' }} />,
  cylinder: <ColumnHeightOutlined style={{ color: '#fa8c16' }} />,
  cone:     <AimOutlined style={{ color: '#eb2f96' }} />,
  torus:    <RetweetOutlined style={{ color: '#722ed1' }} />,
  pad:      <ExpandOutlined style={{ color: '#13c2c2' }} />,
};

type ShapeStats = {
  triangleCount: number;
  volume?: number;
  area?: number;
  valid?: boolean;
  issueCount?: number;
};

/**
 * Feature Tree — a compact list of every CAD shape in `useCadStore`.
 *
 * Per-row actions: visibility toggle, measure (area/volume), validity check,
 * delete. The tree reflects the pure-Rust gfd-cad kernel state; measurements
 * are fetched lazily on click to avoid blocking the render loop.
 */
const FeatureTree: React.FC = () => {
  const shapes = useCadStore((s) => s.shapes);
  const toggle = useCadStore((s) => s.setVisible);
  const remove = useCadStore((s) => s.removeShape);
  const [stats, setStats] = useState<Record<string, ShapeStats>>({});

  const measure = async (id: string) => {
    try {
      const [area, volume, heal] = await Promise.all([
        cadClient.surfaceArea(id).catch(() => ({ area: NaN })),
        cadClient.bboxVolume(id).catch(() => ({ volume: NaN })),
        cadClient.healCheck(id).catch(() => ({ valid: false, issues: [] })),
      ]);
      setStats((prev) => ({
        ...prev,
        [id]: {
          triangleCount: prev[id]?.triangleCount ?? 0,
          area: area.area,
          volume: volume.volume,
          valid: heal.valid,
          issueCount: heal.issues.length,
        },
      }));
      message.success(`${id}: area ${area.area.toFixed(3)}, bbox-V ${volume.volume.toFixed(3)}${heal.valid ? ', ok' : `, ${heal.issues.length} issues`}`);
    } catch (e) {
      message.error(`Measure failed: ${(e as Error).message}`);
    }
  };

  if (shapes.length === 0) {
    return <div style={{ padding: 12, color: '#667', fontSize: 12 }}>Feature tree is empty. Use the ribbon or the Design tab to create shapes.</div>;
  }

  return (
    <div style={{ padding: '4px 6px', color: '#ddd' }}>
      {shapes.map((s) => {
        const icon = KIND_ICON[s.kind] ?? <QuestionCircleOutlined />;
        const stat = stats[s.id];
        const triCount = Math.floor(s.indices.length / 3);
        return (
          <div
            key={s.id}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              padding: '4px 6px',
              borderBottom: '1px solid #252540',
              fontSize: 12,
            }}
          >
            <span style={{ fontSize: 14, width: 18 }}>{icon}</span>
            <span style={{ flex: 1, color: '#eee' }}>{s.id}</span>
            <span style={{ color: '#889', fontSize: 10, minWidth: 60 }}>{s.kind} · {triCount} △</span>
            {stat?.area !== undefined && (
              <Tooltip title={`area ${stat.area.toFixed(3)}, bbox-V ${stat.volume?.toFixed(3)}`}>
                {stat.valid ? (
                  <CheckCircleOutlined style={{ color: '#52c41a', fontSize: 12 }} />
                ) : (
                  <WarningOutlined style={{ color: '#faad14', fontSize: 12 }} />
                )}
              </Tooltip>
            )}
            <Tooltip title="Measure / validity check">
              <Button
                size="small"
                type="text"
                onClick={() => measure(s.id)}
                icon={<span style={{ fontSize: 11 }}>∑</span>}
              />
            </Tooltip>
            <Tooltip title={s.visible ? 'Hide' : 'Show'}>
              <Button
                size="small"
                type="text"
                onClick={() => toggle(s.id, !s.visible)}
                icon={s.visible ? <EyeOutlined /> : <EyeInvisibleOutlined />}
              />
            </Tooltip>
            <Tooltip title="Export STL">
              <Button
                size="small"
                type="text"
                onClick={async () => {
                  const suggested = `${s.id}.stl`;
                  const path = window.prompt('STL output path (absolute or relative to cwd):', suggested);
                  if (!path) return;
                  try {
                    const resp = await cadClient.exportStl(s.id, path, false);
                    message.success(`Exported ${resp.triangle_count} triangles → ${resp.path}`);
                  } catch (e) {
                    message.error(`Export failed: ${(e as Error).message}`);
                  }
                }}
                icon={<ExportOutlined />}
              />
            </Tooltip>
            <Tooltip title="Export STEP (AP214)">
              <Button
                size="small"
                type="text"
                onClick={async () => {
                  const suggested = `${s.id}.stp`;
                  const path = window.prompt('STEP output path:', suggested);
                  if (!path) return;
                  try {
                    const resp = await cadClient.exportStep(s.id, path);
                    message.success(`STEP export → ${resp.path}`);
                  } catch (e) {
                    message.error(`STEP export failed: ${(e as Error).message}`);
                  }
                }}
              >
                STEP
              </Button>
            </Tooltip>
            <Tooltip title="Delete">
              <Button
                size="small"
                type="text"
                danger
                onClick={() => remove(s.id)}
                icon={<DeleteOutlined />}
              />
            </Tooltip>
          </div>
        );
      })}
    </div>
  );
};

export default FeatureTree;

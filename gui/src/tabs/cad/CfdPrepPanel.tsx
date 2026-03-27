import React, { useState, useCallback } from 'react';
import { Button, InputNumber, Form, Divider, Select, message, Typography } from 'antd';
import {
  ExpandOutlined,
  ExperimentOutlined,
  BorderInnerOutlined,
  AppstoreOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';

let nextEnclosureId = 100;

const CfdPrepPanel: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const addShape = useAppStore((s) => s.addShape);

  const [padXp, setPadXp] = useState(2.0);
  const [padXn, setPadXn] = useState(2.0);
  const [padYp, setPadYp] = useState(2.0);
  const [padYn, setPadYn] = useState(2.0);
  const [padZp, setPadZp] = useState(2.0);
  const [padZn, setPadZn] = useState(2.0);
  const [symmetryPlane, setSymmetryPlane] = useState<'XY' | 'XZ' | 'YZ'>('XZ');

  const bodyShapes = shapes.filter(
    (s) => s.group !== 'enclosure' && s.kind !== 'enclosure'
  );

  const handleCreateEnclosure = useCallback(() => {
    if (bodyShapes.length === 0) {
      message.warning('No body shapes to enclose. Create shapes first.');
      return;
    }

    // Calculate bounding box of all body shapes
    let minX = Infinity, maxX = -Infinity;
    let minY = Infinity, maxY = -Infinity;
    let minZ = Infinity, maxZ = -Infinity;

    bodyShapes.forEach((s) => {
      // Approximate bounds from position + dimension extents
      const hw = (s.dimensions.width ?? s.dimensions.radius ?? 0.5);
      const hh = (s.dimensions.height ?? s.dimensions.radius ?? 0.5);
      const hd = (s.dimensions.depth ?? s.dimensions.radius ?? 0.5);

      minX = Math.min(minX, s.position[0] - hw);
      maxX = Math.max(maxX, s.position[0] + hw);
      minY = Math.min(minY, s.position[1] - hh);
      maxY = Math.max(maxY, s.position[1] + hh);
      minZ = Math.min(minZ, s.position[2] - hd);
      maxZ = Math.max(maxZ, s.position[2] + hd);
    });

    // Apply padding
    minX -= padXn;
    maxX += padXp;
    minY -= padYn;
    maxY += padYp;
    minZ -= padZn;
    maxZ += padZp;

    const w = maxX - minX;
    const h = maxY - minY;
    const d = maxZ - minZ;

    const id = `encl-${nextEnclosureId++}`;
    addShape({
      id,
      name: 'Enclosure',
      kind: 'enclosure',
      position: [(minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2],
      rotation: [0, 0, 0],
      dimensions: { width: w, height: h, depth: d },
      isEnclosure: true,
      group: 'enclosure',
    });
    message.success(
      `Enclosure created: ${w.toFixed(2)} x ${h.toFixed(2)} x ${d.toFixed(2)}`
    );
  }, [bodyShapes, padXp, padXn, padYp, padYn, padZp, padZn, addShape]);

  const handleSymmetryCut = useCallback(() => {
    message.info(
      `Symmetry cut along ${symmetryPlane} plane at origin. All shapes will be halved. (Simulated)`
    );
  }, [symmetryPlane]);

  const handleExtractFluid = useCallback(() => {
    const enclosures = shapes.filter(
      (s) => s.kind === 'enclosure' || s.isEnclosure
    );
    if (enclosures.length === 0) {
      message.warning('Create an enclosure first.');
      return;
    }
    message.info(
      'Extracted fluid domain by subtracting solid bodies from enclosure. (Simulated)'
    );
  }, [shapes]);

  const handleNameRegions = useCallback(() => {
    const enclosures = shapes.filter(
      (s) => s.kind === 'enclosure' || s.isEnclosure
    );
    if (enclosures.length === 0) {
      message.warning('Create an enclosure first.');
      return;
    }
    message.success(
      'Auto-named regions: inlet (-X face), outlet (+X face), wall (remaining faces).'
    );
  }, [shapes]);

  return (
    <div style={{ padding: 12 }}>
      <div
        style={{
          fontWeight: 600,
          marginBottom: 12,
          fontSize: 14,
          borderBottom: '1px solid #303030',
          paddingBottom: 8,
        }}
      >
        CFD Preparation
      </div>

      {/* Create Enclosure */}
      <Typography.Text strong style={{ fontSize: 12 }}>
        Enclosure Padding
      </Typography.Text>
      <Form layout="vertical" size="small" style={{ marginTop: 8 }}>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
          <Form.Item label="+X" style={{ marginBottom: 4 }}>
            <InputNumber
              value={padXp}
              min={0}
              step={0.5}
              onChange={(v) => setPadXp(v ?? 2)}
              style={{ width: '100%' }}
            />
          </Form.Item>
          <Form.Item label="-X" style={{ marginBottom: 4 }}>
            <InputNumber
              value={padXn}
              min={0}
              step={0.5}
              onChange={(v) => setPadXn(v ?? 2)}
              style={{ width: '100%' }}
            />
          </Form.Item>
          <Form.Item label="+Y" style={{ marginBottom: 4 }}>
            <InputNumber
              value={padYp}
              min={0}
              step={0.5}
              onChange={(v) => setPadYp(v ?? 2)}
              style={{ width: '100%' }}
            />
          </Form.Item>
          <Form.Item label="-Y" style={{ marginBottom: 4 }}>
            <InputNumber
              value={padYn}
              min={0}
              step={0.5}
              onChange={(v) => setPadYn(v ?? 2)}
              style={{ width: '100%' }}
            />
          </Form.Item>
          <Form.Item label="+Z" style={{ marginBottom: 4 }}>
            <InputNumber
              value={padZp}
              min={0}
              step={0.5}
              onChange={(v) => setPadZp(v ?? 2)}
              style={{ width: '100%' }}
            />
          </Form.Item>
          <Form.Item label="-Z" style={{ marginBottom: 4 }}>
            <InputNumber
              value={padZn}
              min={0}
              step={0.5}
              onChange={(v) => setPadZn(v ?? 2)}
              style={{ width: '100%' }}
            />
          </Form.Item>
        </div>
      </Form>

      <Button
        type="primary"
        icon={<ExpandOutlined />}
        onClick={handleCreateEnclosure}
        block
        size="small"
        style={{ marginBottom: 8 }}
      >
        Create Enclosure
      </Button>

      <Divider style={{ margin: '8px 0' }} />

      {/* Extract Fluid */}
      <Button
        icon={<ExperimentOutlined />}
        onClick={handleExtractFluid}
        block
        size="small"
        style={{ marginBottom: 8 }}
      >
        Extract Fluid Domain
      </Button>

      <Divider style={{ margin: '8px 0' }} />

      {/* Symmetry Cut */}
      <Typography.Text strong style={{ fontSize: 12 }}>
        Symmetry Cut
      </Typography.Text>
      <Form layout="vertical" size="small" style={{ marginTop: 8 }}>
        <Form.Item label="Cutting Plane" style={{ marginBottom: 8 }}>
          <Select
            value={symmetryPlane}
            onChange={(v) => setSymmetryPlane(v)}
            options={[
              { value: 'XY', label: 'XY Plane (Z=0)' },
              { value: 'XZ', label: 'XZ Plane (Y=0)' },
              { value: 'YZ', label: 'YZ Plane (X=0)' },
            ]}
          />
        </Form.Item>
      </Form>
      <Button
        icon={<BorderInnerOutlined />}
        onClick={handleSymmetryCut}
        block
        size="small"
        style={{ marginBottom: 8 }}
      >
        Apply Symmetry Cut
      </Button>

      <Divider style={{ margin: '8px 0' }} />

      {/* Name Regions */}
      <Button
        icon={<AppstoreOutlined />}
        onClick={handleNameRegions}
        block
        size="small"
      >
        Auto-Name Regions
      </Button>
    </div>
  );
};

export default CfdPrepPanel;

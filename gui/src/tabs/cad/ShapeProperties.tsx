import React from 'react';
import { Typography, Button, Divider, Tag, message, Switch } from 'antd';
import { DeleteOutlined, LockOutlined, UnlockOutlined } from '@ant-design/icons';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';

/** Dimension field definitions per shape kind */
const kindDimensionLabels: Record<string, Record<string, string>> = {
  box: { width: 'Width', height: 'Height', depth: 'Depth' },
  sphere: { radius: 'Radius' },
  cylinder: { radius: 'Radius', height: 'Height' },
  cone: { radius: 'Base Radius', height: 'Height' },
  torus: { majorRadius: 'Major Radius', minorRadius: 'Minor Radius' },
  pipe: { outerRadius: 'Outer Radius', innerRadius: 'Inner Radius', height: 'Height' },
  enclosure: { width: 'Width', height: 'Height', depth: 'Depth' },
};

const kindTagColors: Record<string, string> = {
  box: 'blue',
  sphere: 'green',
  cylinder: 'orange',
  cone: 'magenta',
  torus: 'purple',
  pipe: 'cyan',
  stl: 'gold',
  enclosure: 'lime',
};

const ShapeProperties: React.FC = () => {
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const shapes = useAppStore((s) => s.shapes);
  const updateShape = useAppStore((s) => s.updateShape);
  const removeShape = useAppStore((s) => s.removeShape);

  const shape = shapes.find((s) => s.id === selectedShapeId);

  if (!shape) {
    return (
      <div style={{ padding: 16, color: '#888' }}>
        <Typography.Text type="secondary">
          Select a shape to edit its properties.
        </Typography.Text>
      </div>
    );
  }

  const isStl = shape.kind === 'stl';

  const baseFields: PropertyField[] = [
    { key: 'name', label: 'Name', type: 'string' },
    { key: 'position', label: 'Position', type: 'vector3', step: 0.1 },
    { key: 'rotation', label: 'Rotation (deg)', type: 'vector3', step: 1 },
  ];

  // Build dimension fields using kind-specific labels
  const labelMap = kindDimensionLabels[shape.kind] ?? {};
  const dimFields: PropertyField[] = isStl
    ? []
    : Object.keys(shape.dimensions).map((k) => ({
        key: `dim_${k}`,
        label: labelMap[k] ?? k.charAt(0).toUpperCase() + k.slice(1),
        type: 'number' as const,
        min: 0.001,
        step: 0.1,
      }));

  const values: Record<string, unknown> = {
    name: shape.name,
    position: shape.position,
    rotation: shape.rotation,
    ...Object.fromEntries(
      Object.entries(shape.dimensions).map(([k, v]) => [`dim_${k}`, v])
    ),
  };

  const handleChange = (key: string, value: unknown) => {
    if (key === 'name') {
      updateShape(shape.id, { name: value as string });
    } else if (key === 'position') {
      updateShape(shape.id, { position: value as [number, number, number] });
    } else if (key === 'rotation') {
      updateShape(shape.id, { rotation: value as [number, number, number] });
    } else if (key.startsWith('dim_')) {
      const dimKey = key.replace('dim_', '');
      let numVal = value as number;
      // Enforce positive dimensions
      if (typeof numVal === 'number' && numVal < 0.001) {
        numVal = 0.001;
      }
      // Pipe: enforce innerRadius < outerRadius
      if (shape.kind === 'pipe') {
        const newDims = { ...shape.dimensions, [dimKey]: numVal };
        if (dimKey === 'innerRadius' && numVal >= (newDims.outerRadius ?? 0.4)) {
          message.warning('Inner radius must be less than outer radius');
          numVal = (newDims.outerRadius ?? 0.4) - 0.01;
        }
        if (dimKey === 'outerRadius' && numVal <= (newDims.innerRadius ?? 0.3)) {
          message.warning('Outer radius must be greater than inner radius');
          numVal = (newDims.innerRadius ?? 0.3) + 0.01;
        }
      }
      // Torus: enforce minorRadius < majorRadius
      if (shape.kind === 'torus') {
        const newDims = { ...shape.dimensions, [dimKey]: numVal };
        if (dimKey === 'minorRadius' && numVal >= (newDims.majorRadius ?? 0.5)) {
          message.warning('Minor radius must be less than major radius');
          numVal = (newDims.majorRadius ?? 0.5) - 0.01;
        }
      }
      updateShape(shape.id, {
        dimensions: { ...shape.dimensions, [dimKey]: numVal },
      });
    }
  };

  return (
    <div>
      <div style={{ padding: '8px 12px', borderBottom: '1px solid #303030' }}>
        <Tag color={kindTagColors[shape.kind] ?? 'default'}>
          {shape.kind.toUpperCase()}
        </Tag>
        {shape.isEnclosure && (
          <Tag color="lime" style={{ marginLeft: 4 }}>
            ENCLOSURE
          </Tag>
        )}
        {shape.booleanRef && (
          <Tag color="gold" style={{ marginLeft: 4 }}>
            BOOLEAN
          </Tag>
        )}
      </div>

      <PropertyGrid
        title={`${shape.kind.charAt(0).toUpperCase() + shape.kind.slice(1)} Properties`}
        fields={[...baseFields, ...dimFields]}
        values={values}
        onChange={handleChange}
      />

      {/* STL-specific read-only info */}
      {isStl && shape.stlData && (
        <div style={{ padding: '0 12px 8px' }}>
          <Divider style={{ margin: '4px 0 8px' }} />
          <div style={{ fontSize: 12, color: '#999' }}>
            <div>
              <strong>Vertices:</strong> {shape.stlData.vertices.length / 3}
            </div>
            <div>
              <strong>Triangles:</strong> {shape.stlData.faceCount}
            </div>
          </div>
        </div>
      )}

      {/* Pipe validation hint */}
      {shape.kind === 'pipe' && (
        <div style={{ padding: '0 12px 8px', fontSize: 11, color: '#faad14' }}>
          Inner radius must be smaller than outer radius.
        </div>
      )}

      <Divider style={{ margin: '4px 12px' }} />
      <div style={{ padding: '0 12px 8px', display: 'flex', alignItems: 'center', gap: 8 }}>
        <Switch
          size="small"
          checked={shape.locked ?? false}
          onChange={(checked) => updateShape(shape.id, { locked: checked })}
          checkedChildren={<LockOutlined />}
          unCheckedChildren={<UnlockOutlined />}
        />
        <span style={{ fontSize: 11, color: shape.locked ? '#faad14' : '#667' }}>
          {shape.locked ? 'Locked (cannot delete/move)' : 'Unlocked'}
        </span>
      </div>
      <div style={{ padding: '0 12px 12px' }}>
        <Button
          danger
          block
          icon={<DeleteOutlined />}
          onClick={() => {
            if (shape.locked) { message.warning('Shape is locked. Unlock it first.'); return; }
            removeShape(shape.id);
          }}
          disabled={shape.locked}
        >
          Delete Shape
        </Button>
      </div>
    </div>
  );
};

export default ShapeProperties;

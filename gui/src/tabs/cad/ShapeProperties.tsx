import React from 'react';
import { Typography, Button, Divider } from 'antd';
import { DeleteOutlined } from '@ant-design/icons';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';

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

  const baseFields: PropertyField[] = [
    { key: 'name', label: 'Name', type: 'string' },
    { key: 'position', label: 'Position', type: 'vector3', step: 0.1 },
    { key: 'rotation', label: 'Rotation (deg)', type: 'vector3', step: 1 },
  ];

  const dimFields: PropertyField[] = Object.keys(shape.dimensions).map(
    (k) => ({
      key: `dim_${k}`,
      label: k.charAt(0).toUpperCase() + k.slice(1),
      type: 'number' as const,
      min: 0.001,
      step: 0.1,
    })
  );

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
      updateShape(shape.id, {
        dimensions: { ...shape.dimensions, [dimKey]: value as number },
      });
    }
  };

  return (
    <div>
      <PropertyGrid
        title={`${shape.kind.toUpperCase()} Properties`}
        fields={[...baseFields, ...dimFields]}
        values={values}
        onChange={handleChange}
      />
      <Divider style={{ margin: '4px 12px' }} />
      <div style={{ padding: '0 12px 12px' }}>
        <Button
          danger
          block
          icon={<DeleteOutlined />}
          onClick={() => removeShape(shape.id)}
        >
          Delete Shape
        </Button>
      </div>
    </div>
  );
};

export default ShapeProperties;

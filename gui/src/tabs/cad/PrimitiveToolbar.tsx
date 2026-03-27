import React from 'react';
import { Button, Space, Upload, Tooltip } from 'antd';
import {
  BorderOutlined,
  RadiusSettingOutlined,
  ColumnHeightOutlined,
  ImportOutlined,
  PlusCircleOutlined,
  MinusCircleOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';
import type { ShapeKind } from '../../store/useAppStore';

let nextId = 1;

function makeShape(kind: ShapeKind) {
  const id = `shape-${nextId++}`;
  const defaults: Record<ShapeKind, Record<string, number>> = {
    box: { width: 1, height: 1, depth: 1 },
    sphere: { radius: 0.5 },
    cylinder: { radius: 0.3, height: 1 },
  };
  return {
    id,
    name: `${kind}-${id}`,
    kind,
    position: [0, 0, 0] as [number, number, number],
    rotation: [0, 0, 0] as [number, number, number],
    dimensions: { ...defaults[kind] },
  };
}

const PrimitiveToolbar: React.FC = () => {
  const addShape = useAppStore((s) => s.addShape);

  const create = (kind: ShapeKind) => {
    const shape = makeShape(kind);
    addShape(shape);
  };

  return (
    <div
      style={{
        padding: '8px 12px',
        borderBottom: '1px solid #303030',
        background: '#1f1f1f',
      }}
    >
      <Space wrap>
        <Tooltip title="Box">
          <Button icon={<BorderOutlined />} onClick={() => create('box')}>
            Box
          </Button>
        </Tooltip>
        <Tooltip title="Sphere">
          <Button
            icon={<RadiusSettingOutlined />}
            onClick={() => create('sphere')}
          >
            Sphere
          </Button>
        </Tooltip>
        <Tooltip title="Cylinder">
          <Button
            icon={<ColumnHeightOutlined />}
            onClick={() => create('cylinder')}
          >
            Cylinder
          </Button>
        </Tooltip>
        <Upload accept=".stl" showUploadList={false} beforeUpload={() => false}>
          <Tooltip title="Import STL">
            <Button icon={<ImportOutlined />}>Import STL</Button>
          </Tooltip>
        </Upload>
        <Tooltip title="Union (Boolean)">
          <Button icon={<PlusCircleOutlined />}>Union</Button>
        </Tooltip>
        <Tooltip title="Subtract (Boolean)">
          <Button icon={<MinusCircleOutlined />}>Subtract</Button>
        </Tooltip>
      </Space>
    </div>
  );
};

export default PrimitiveToolbar;

import React, { useCallback } from 'react';
import {
  BorderOutlined,
  RadiusSettingOutlined,
  ColumnHeightOutlined,
} from '@ant-design/icons';
import OutlineTree from '../../components/OutlineTree';
import type { TreeItem } from '../../components/OutlineTree';
import { useAppStore } from '../../store/useAppStore';
import type { ShapeKind } from '../../store/useAppStore';

const kindColors: Record<ShapeKind, string> = {
  box: '#1677ff',      // blue
  sphere: '#52c41a',   // green
  cylinder: '#fa8c16', // orange
};

function kindIcon(kind: ShapeKind) {
  const color = kindColors[kind];
  switch (kind) {
    case 'box':
      return <BorderOutlined style={{ color }} />;
    case 'sphere':
      return <RadiusSettingOutlined style={{ color }} />;
    case 'cylinder':
      return <ColumnHeightOutlined style={{ color }} />;
  }
}

const CadTree: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const selectShape = useAppStore((s) => s.selectShape);
  const removeShape = useAppStore((s) => s.removeShape);

  const items: TreeItem[] = [
    {
      key: 'bodies',
      title: `Bodies (${shapes.length})`,
      children: shapes.map((s) => ({
        key: s.id,
        title: s.name,
        icon: kindIcon(s.kind),
        isLeaf: true,
      })),
    },
  ];

  const handleRename = useCallback(
    (key: string) => {
      const shape = shapes.find((s) => s.id === key);
      if (!shape) return;
      const newName = prompt('Rename shape:', shape.name);
      if (newName && newName !== shape.name) {
        useAppStore.getState().updateShape(key, { name: newName });
      }
    },
    [shapes]
  );

  return (
    <div>
      <div
        style={{
          padding: '8px 12px',
          fontWeight: 600,
          borderBottom: '1px solid #303030',
        }}
      >
        CAD Tree
      </div>
      <OutlineTree
        items={items}
        selectedKey={selectedShapeId}
        onSelect={selectShape}
        onDelete={removeShape}
        onRename={handleRename}
      />
    </div>
  );
};

export default CadTree;

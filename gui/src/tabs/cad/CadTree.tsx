import React, { useCallback } from 'react';
import {
  BorderOutlined,
  RadiusSettingOutlined,
  ColumnHeightOutlined,
} from '@ant-design/icons';
import OutlineTree from '../../components/OutlineTree';
import type { TreeItem } from '../../components/OutlineTree';
import { useAppStore } from '../../store/useAppStore';

const kindIcons = {
  box: <BorderOutlined />,
  sphere: <RadiusSettingOutlined />,
  cylinder: <ColumnHeightOutlined />,
};

const CadTree: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const selectShape = useAppStore((s) => s.selectShape);
  const removeShape = useAppStore((s) => s.removeShape);

  const items: TreeItem[] = [
    {
      key: 'bodies',
      title: 'Bodies',
      children: shapes.map((s) => ({
        key: s.id,
        title: s.name,
        icon: kindIcons[s.kind],
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
    <OutlineTree
      items={items}
      selectedKey={selectedShapeId}
      onSelect={selectShape}
      onDelete={removeShape}
      onRename={handleRename}
    />
  );
};

export default CadTree;

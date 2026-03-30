import React, { useCallback, useMemo } from 'react';
import {
  BorderOutlined,
  RadiusSettingOutlined,
  ColumnHeightOutlined,
  AimOutlined,
  RetweetOutlined,
  GatewayOutlined,
  FileOutlined,
  ExpandOutlined,
  InteractionOutlined,
  EyeInvisibleOutlined,
} from '@ant-design/icons';
import OutlineTree from '../../components/OutlineTree';
import type { TreeItem } from '../../components/OutlineTree';
import { useAppStore } from '../../store/useAppStore';
import type { ShapeKind } from '../../store/useAppStore';

const kindColors: Record<string, string> = {
  box: '#1677ff',
  sphere: '#52c41a',
  cylinder: '#fa8c16',
  cone: '#eb2f96',
  torus: '#722ed1',
  pipe: '#13c2c2',
  stl: '#d4b106',
  enclosure: '#52c41a',
};

function kindIcon(kind: ShapeKind, isEnclosure?: boolean) {
  if (isEnclosure || kind === 'enclosure') {
    return <ExpandOutlined style={{ color: kindColors.enclosure }} />;
  }
  const color = kindColors[kind] || '#888';
  switch (kind) {
    case 'box':
      return <BorderOutlined style={{ color }} />;
    case 'sphere':
      return <RadiusSettingOutlined style={{ color }} />;
    case 'cylinder':
      return <ColumnHeightOutlined style={{ color }} />;
    case 'cone':
      return <AimOutlined style={{ color }} />;
    case 'torus':
      return <RetweetOutlined style={{ color }} />;
    case 'pipe':
      return <GatewayOutlined style={{ color }} />;
    case 'stl':
      return <FileOutlined style={{ color }} />;
    default:
      return <BorderOutlined style={{ color }} />;
  }
}

const CadTree: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const booleanOps = useAppStore((s) => s.booleanOps);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const selectShape = useAppStore((s) => s.selectShape);
  const removeShape = useAppStore((s) => s.removeShape);

  const { bodies, booleans, enclosures } = useMemo(() => {
    const bodies = shapes.filter(
      (s) => s.group !== 'enclosure' && s.group !== 'boolean'
    );
    const booleans = shapes.filter((s) => s.group === 'boolean');
    const enclosures = shapes.filter(
      (s) => s.group === 'enclosure' || s.kind === 'enclosure'
    );
    return { bodies, booleans, enclosures };
  }, [shapes]);

  const items: TreeItem[] = [
    {
      key: 'bodies',
      title: `Bodies (${bodies.length})`,
      children: bodies.map((s) => ({
        key: s.id,
        title: s.visible === false
          ? <span style={{ opacity: 0.4 }}><EyeInvisibleOutlined style={{ marginRight: 4, fontSize: 10 }} />{s.name}</span>
          : s.name,
        icon: kindIcon(s.kind),
        isLeaf: true,
      })),
    },
  ];

  if (booleanOps.length > 0 || booleans.length > 0) {
    items.push({
      key: 'booleans',
      title: `Boolean Operations (${booleanOps.length})`,
      icon: <InteractionOutlined style={{ color: '#faad14' }} />,
      children: booleanOps.map((op) => ({
        key: `boolop-${op.id}`,
        title: op.name,
        icon: <InteractionOutlined style={{ color: '#faad14' }} />,
        isLeaf: true,
      })),
    });
  }

  if (enclosures.length > 0) {
    items.push({
      key: 'enclosures',
      title: `Enclosures (${enclosures.length})`,
      icon: <ExpandOutlined style={{ color: '#52c41a' }} />,
      children: enclosures.map((s) => ({
        key: s.id,
        title: s.visible === false
          ? <span style={{ opacity: 0.4 }}><EyeInvisibleOutlined style={{ marginRight: 4, fontSize: 10 }} />{s.name}</span>
          : s.name,
        icon: kindIcon(s.kind, true),
        isLeaf: true,
      })),
    });
  }

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

  const handleSelect = useCallback(
    (key: string) => {
      // Only select actual shape keys (not group headers or boolean op keys)
      if (key.startsWith('boolop-') || key === 'bodies' || key === 'booleans' || key === 'enclosures') {
        return;
      }
      selectShape(key);
    },
    [selectShape]
  );

  const handleDelete = useCallback(
    (key: string) => {
      if (key.startsWith('boolop-')) {
        const opId = key.replace('boolop-', '');
        useAppStore.getState().removeBooleanOp(opId);
        return;
      }
      removeShape(key);
    },
    [removeShape]
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
        onSelect={handleSelect}
        onDelete={handleDelete}
        onRename={handleRename}
      />
    </div>
  );
};

export default CadTree;

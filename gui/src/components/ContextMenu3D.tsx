import React, { useCallback, useEffect } from 'react';
import { message } from 'antd';
import {
  DeleteOutlined,
  CopyOutlined,
  EyeInvisibleOutlined,
  InfoCircleOutlined,
  BorderOutlined,
  RadiusSettingOutlined,
  CompressOutlined,
  SyncOutlined,
  ImportOutlined,
  ExportOutlined,
  GatewayOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../store/useAppStore';

let nextContextId = 500;

const ContextMenu3D: React.FC = () => {
  const contextMenu = useAppStore((s) => s.contextMenu);
  const setContextMenu = useAppStore((s) => s.setContextMenu);
  const removeShape = useAppStore((s) => s.removeShape);
  const toggleShapeVisibility = useAppStore((s) => s.toggleShapeVisibility);
  const addShape = useAppStore((s) => s.addShape);
  const shapes = useAppStore((s) => s.shapes);
  const selectShape = useAppStore((s) => s.selectShape);
  const addNamedSelection = useAppStore((s) => s.addNamedSelection);

  // Close on escape or any other click
  useEffect(() => {
    if (!contextMenu) return;
    const handleClose = () => setContextMenu(null);
    // Add listeners immediately but on next microtask to avoid the triggering click
    const raf = requestAnimationFrame(() => {
      window.addEventListener('click', handleClose);
      window.addEventListener('keydown', handleClose);
    });
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener('click', handleClose);
      window.removeEventListener('keydown', handleClose);
    };
  }, [contextMenu, setContextMenu]);

  const handleAction = useCallback(
    (action: () => void) => {
      action();
      setContextMenu(null);
    },
    [setContextMenu]
  );

  if (!contextMenu) return null;

  const { x, y, shapeId } = contextMenu;
  const shape = shapeId ? shapes.find((s) => s.id === shapeId) : null;

  // Build menu items based on context
  interface MenuItem {
    key: string;
    icon: React.ReactNode;
    label: string;
    action: () => void;
    danger?: boolean;
    divider?: boolean;
  }

  const items: MenuItem[] = [];

  if (shape) {
    // Shape-specific actions
    items.push({
      key: 'delete',
      icon: <DeleteOutlined />,
      label: 'Delete',
      danger: true,
      action: () => { removeShape(shape.id); message.info(`Deleted ${shape.name}`); },
    });
    items.push({
      key: 'duplicate',
      icon: <CopyOutlined />,
      label: 'Duplicate',
      action: () => {
        const id = `shape-${nextContextId++}`;
        addShape({
          ...shape,
          id,
          name: `${shape.name}-copy`,
          position: [shape.position[0] + 0.5, shape.position[1], shape.position[2]],
          stlData: shape.stlData,
        });
        selectShape(id);
        message.success(`Duplicated ${shape.name}`);
      },
    });
    items.push({
      key: 'hide',
      icon: <EyeInvisibleOutlined />,
      label: 'Hide',
      action: () => { toggleShapeVisibility(shape.id); message.info(`Hidden ${shape.name}`); },
    });
    items.push({
      key: 'properties',
      icon: <InfoCircleOutlined />,
      label: 'Properties',
      action: () => {
        selectShape(shape.id);
        message.info(`Properties: ${shape.kind} [${shape.position.map(v => v.toFixed(2)).join(', ')}]`);
      },
    });
    items.push({
      key: 'set-position',
      icon: <InfoCircleOutlined />,
      label: 'Set Position...',
      action: () => {
        const input = prompt(`Position (X, Y, Z):`, shape.position.map(v => v.toFixed(3)).join(', '));
        if (!input) return;
        const parts = input.split(',').map(s => parseFloat(s.trim()));
        if (parts.length === 3 && parts.every(v => !isNaN(v))) {
          useAppStore.getState().updateShape(shape.id, { position: parts as [number, number, number] });
          message.success(`Position set to (${parts.map(v => v.toFixed(3)).join(', ')})`);
        } else {
          message.error('Invalid input. Use format: X, Y, Z');
        }
      },
    });
    // Divider
    items.push({ key: 'div1', icon: null, label: '', action: () => {}, divider: true });

    // CFD boundary assignment
    items.push({
      key: 'set-inlet',
      icon: <ImportOutlined />,
      label: 'Set as Inlet',
      action: () => {
        addNamedSelection({
          name: `inlet-${shape.name}`,
          type: 'inlet',
          faces: [0],
          center: shape.position,
          normal: [-1, 0, 0],
          width: 1,
          height: 1,
          color: '#1677ff',
        });
        message.success(`${shape.name} set as Inlet`);
      },
    });
    items.push({
      key: 'set-outlet',
      icon: <ExportOutlined />,
      label: 'Set as Outlet',
      action: () => {
        addNamedSelection({
          name: `outlet-${shape.name}`,
          type: 'outlet',
          faces: [1],
          center: shape.position,
          normal: [1, 0, 0],
          width: 1,
          height: 1,
          color: '#ff4d4f',
        });
        message.success(`${shape.name} set as Outlet`);
      },
    });
    items.push({
      key: 'set-wall',
      icon: <GatewayOutlined />,
      label: 'Set as Wall',
      action: () => {
        addNamedSelection({
          name: `wall-${shape.name}`,
          type: 'wall',
          faces: [2],
          center: shape.position,
          normal: [0, 1, 0],
          width: 1,
          height: 1,
          color: '#8c8c8c',
        });
        message.success(`${shape.name} set as Wall`);
      },
    });
  } else {
    // No shape selected — general viewport actions
    items.push({
      key: 'create-box',
      icon: <BorderOutlined />,
      label: 'Create Box',
      action: () => {
        const id = `shape-${nextContextId++}`;
        addShape({
          id,
          name: `box-${id}`,
          kind: 'box',
          position: [0, 0, 0],
          rotation: [0, 0, 0],
          dimensions: { width: 1, height: 1, depth: 1 },
          group: 'body',
        });
        selectShape(id);
        message.success('Box created');
      },
    });
    items.push({
      key: 'create-sphere',
      icon: <RadiusSettingOutlined />,
      label: 'Create Sphere',
      action: () => {
        const id = `shape-${nextContextId++}`;
        addShape({
          id,
          name: `sphere-${id}`,
          kind: 'sphere',
          position: [0, 0, 0],
          rotation: [0, 0, 0],
          dimensions: { radius: 0.5 },
          group: 'body',
        });
        selectShape(id);
        message.success('Sphere created');
      },
    });
    items.push({ key: 'div2', icon: null, label: '', action: () => {}, divider: true });
    items.push({
      key: 'zoom-fit',
      icon: <CompressOutlined />,
      label: 'Zoom to Fit',
      action: () => {
        window.dispatchEvent(new CustomEvent('gfd-camera-preset', { detail: { position: [5, 5, 5] } }));
      },
    });
    items.push({
      key: 'reset-view',
      icon: <SyncOutlined />,
      label: 'Reset View',
      action: () => {
        window.dispatchEvent(new CustomEvent('gfd-camera-preset', { detail: { position: [5, 5, 5] } }));
      },
    });
    items.push({ key: 'div3', icon: null, label: '', action: () => {}, divider: true });
    items.push({
      key: 'add-note',
      icon: <InfoCircleOutlined />,
      label: 'Add Note Here',
      action: () => {
        const text = prompt('Enter annotation text:', 'Note');
        if (text) {
          useAppStore.getState().addAnnotation(text, [0, 0, 0]);
          message.success(`Note added: "${text}"`);
        }
      },
    });
  }

  // Shape-specific: add note at shape position + array pattern
  if (shape) {
    items.push({ key: 'div-note', icon: null, label: '', action: () => {}, divider: true });
    items.push({
      key: 'add-note-at',
      icon: <InfoCircleOutlined />,
      label: 'Add Note at Shape',
      action: () => {
        const text = prompt('Enter annotation text:', shape.name);
        if (text) {
          useAppStore.getState().addAnnotation(text, [...shape.position] as [number, number, number]);
          message.success(`Note added: "${text}"`);
        }
      },
    });
    items.push({
      key: 'array-pattern',
      icon: <CopyOutlined />,
      label: 'Array Pattern...',
      action: () => {
        const countStr = prompt('Number of copies:', '3');
        if (!countStr) return;
        const count = parseInt(countStr) || 3;
        const spacingStr = prompt('Spacing (m):', '1.0');
        const spacing = parseFloat(spacingStr ?? '1') || 1;
        const axisStr = prompt('Axis (x/y/z):', 'x');
        const axis = (axisStr ?? 'x').toLowerCase();
        const axisIdx = axis === 'y' ? 1 : axis === 'z' ? 2 : 0;
        for (let i = 1; i <= count; i++) {
          const id = `shape-arr-${Date.now()}-${i}`;
          const newPos: [number, number, number] = [...shape.position];
          newPos[axisIdx] += spacing * i;
          addShape({
            ...shape,
            id,
            name: `${shape.name}-${i}`,
            position: newPos,
            locked: false,
            stlData: shape.stlData,
          });
        }
        message.success(`Created ${count} copies along ${axis.toUpperCase()} with ${spacing}m spacing`);
      },
    });
  }

  // Position the menu, keeping it within viewport
  const menuWidth = 200;
  const menuHeight = items.length * 30 + 8;
  const adjustedX = x + menuWidth > window.innerWidth ? window.innerWidth - menuWidth - 4 : x;
  const adjustedY = y + menuHeight > window.innerHeight ? window.innerHeight - menuHeight - 4 : y;

  return (
    <div
      style={{
        position: 'fixed',
        left: adjustedX,
        top: adjustedY,
        width: menuWidth,
        background: '#1a1a2e',
        border: '1px solid #303050',
        borderRadius: 6,
        padding: '4px 0',
        zIndex: 1000,
        boxShadow: '0 4px 16px rgba(0,0,0,0.6)',
      }}
    >
      {items.map((item) => {
        if (item.divider) {
          return (
            <div
              key={item.key}
              style={{ height: 1, background: '#303050', margin: '3px 8px' }}
            />
          );
        }
        return (
          <div
            key={item.key}
            onClick={(e) => {
              e.stopPropagation();
              handleAction(item.action);
            }}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
              padding: '5px 12px',
              cursor: 'pointer',
              color: item.danger ? '#ff4d4f' : '#bbc',
              fontSize: 12,
              transition: 'background 0.1s',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = '#252540';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
            }}
          >
            <span style={{ fontSize: 13, width: 16, textAlign: 'center', color: item.danger ? '#ff4d4f' : '#889' }}>
              {item.icon}
            </span>
            {item.label}
          </div>
        );
      })}
    </div>
  );
};

export default ContextMenu3D;

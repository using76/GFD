import React, { useMemo } from 'react';
import { Tree, Dropdown } from 'antd';
import type { TreeDataNode } from 'antd';
import {
  FolderOutlined,
  FileOutlined,
  DeleteOutlined,
  EditOutlined,
} from '@ant-design/icons';

export interface TreeItem {
  key: string;
  title: React.ReactNode;
  icon?: React.ReactNode;
  children?: TreeItem[];
  isLeaf?: boolean;
}

interface OutlineTreeProps {
  items: TreeItem[];
  selectedKey?: string | null;
  onSelect?: (key: string) => void;
  onDelete?: (key: string) => void;
  onRename?: (key: string) => void;
  onDoubleClick?: (key: string) => void;
}

function toAntTreeData(items: TreeItem[]): TreeDataNode[] {
  return items.map((item) => ({
    key: item.key,
    title: item.title,
    icon: item.icon ?? (item.children ? <FolderOutlined /> : <FileOutlined />),
    isLeaf: item.isLeaf ?? !item.children,
    children: item.children ? toAntTreeData(item.children) : undefined,
  }));
}

const OutlineTree: React.FC<OutlineTreeProps> = ({
  items,
  selectedKey,
  onSelect,
  onDelete,
  onRename,
  onDoubleClick,
}) => {
  const treeData = useMemo(() => toAntTreeData(items), [items]);

  const contextMenuItems = (nodeKey: string) => {
    const menuItems = [];
    if (onRename) {
      menuItems.push({
        key: 'rename',
        icon: <EditOutlined />,
        label: 'Rename',
        onClick: () => onRename(nodeKey),
      });
    }
    if (onDelete) {
      menuItems.push({
        key: 'delete',
        icon: <DeleteOutlined />,
        label: 'Delete',
        danger: true,
        onClick: () => onDelete(nodeKey),
      });
    }
    return menuItems;
  };

  return (
    <Tree
      showIcon
      defaultExpandAll
      selectedKeys={selectedKey ? [selectedKey] : []}
      treeData={treeData}
      onSelect={(keys) => {
        if (keys.length > 0 && onSelect) {
          onSelect(keys[0] as string);
        }
      }}
      titleRender={(node) => {
        const key = node.key as string;
        const dblHandler = onDoubleClick ? () => onDoubleClick(key) : undefined;
        const hasMenu = onDelete || onRename;
        if (!hasMenu) {
          return <span onDoubleClick={dblHandler}>{node.title as string}</span>;
        }
        return (
          <Dropdown
            menu={{ items: contextMenuItems(key) }}
            trigger={['contextMenu']}
          >
            <span onDoubleClick={dblHandler}>{node.title as string}</span>
          </Dropdown>
        );
      }}
      style={{ padding: 8 }}
    />
  );
};

export default OutlineTree;

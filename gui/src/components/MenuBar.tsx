import { Menu } from 'antd';
import type { MenuProps } from 'antd';
import {
  FileOutlined,
  EditOutlined,
  EyeOutlined,
  QuestionCircleOutlined,
  FolderOpenOutlined,
  SaveOutlined,
  ExportOutlined,
  UndoOutlined,
  RedoOutlined,
  SettingOutlined,
  InfoCircleOutlined,
} from '@ant-design/icons';

type MenuItem = Required<MenuProps>['items'][number];

const menuItems: MenuItem[] = [
  {
    key: 'file',
    label: 'File',
    icon: <FileOutlined />,
    children: [
      { key: 'file:new', label: 'New Project', icon: <FileOutlined /> },
      { key: 'file:open', label: 'Open...', icon: <FolderOpenOutlined /> },
      { key: 'file:save', label: 'Save', icon: <SaveOutlined /> },
      { key: 'file:saveas', label: 'Save As...' },
      { type: 'divider' },
      { key: 'file:import', label: 'Import Mesh...' },
      { key: 'file:export', label: 'Export VTK...', icon: <ExportOutlined /> },
      { type: 'divider' },
      { key: 'file:exit', label: 'Exit' },
    ],
  },
  {
    key: 'edit',
    label: 'Edit',
    icon: <EditOutlined />,
    children: [
      { key: 'edit:undo', label: 'Undo', icon: <UndoOutlined /> },
      { key: 'edit:redo', label: 'Redo', icon: <RedoOutlined /> },
      { type: 'divider' },
      { key: 'edit:preferences', label: 'Preferences...', icon: <SettingOutlined /> },
    ],
  },
  {
    key: 'view',
    label: 'View',
    icon: <EyeOutlined />,
    children: [
      { key: 'view:wireframe', label: 'Wireframe' },
      { key: 'view:solid', label: 'Solid' },
      { key: 'view:contour', label: 'Contour' },
      { type: 'divider' },
      { key: 'view:perspective', label: 'Perspective Camera' },
      { key: 'view:orthographic', label: 'Orthographic Camera' },
      { type: 'divider' },
      { key: 'view:fitall', label: 'Fit All' },
    ],
  },
  {
    key: 'help',
    label: 'Help',
    icon: <QuestionCircleOutlined />,
    children: [
      { key: 'help:docs', label: 'Documentation' },
      { key: 'help:about', label: 'About GFD', icon: <InfoCircleOutlined /> },
    ],
  },
];

export default function MenuBar() {
  const handleClick: MenuProps['onClick'] = (info) => {
    console.log('[Menu]', info.key);
    // Menu actions will be wired up as features are implemented
  };

  return (
    <Menu
      mode="horizontal"
      items={menuItems}
      onClick={handleClick}
      style={{
        background: 'transparent',
        borderBottom: 'none',
        lineHeight: '38px',
        minWidth: 0,
      }}
      selectable={false}
    />
  );
}

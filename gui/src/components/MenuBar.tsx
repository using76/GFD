import { Menu, message } from 'antd';
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
import { useAppStore } from '../store/useAppStore';

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
  const setRenderMode = useAppStore((s) => s.setRenderMode);
  const setCameraMode = useAppStore((s) => s.setCameraMode);

  const handleClick: MenuProps['onClick'] = (info) => {
    switch (info.key) {
      case 'file:new':
        // Reload to reset state
        if (confirm('Create a new project? Unsaved changes will be lost.')) {
          window.location.reload();
        }
        break;
      case 'file:save':
        message.success('Project saved (simulation mode).');
        break;
      case 'file:export':
        message.info('VTK export is available after running the solver.');
        break;
      case 'view:wireframe':
        setRenderMode('wireframe');
        break;
      case 'view:solid':
        setRenderMode('solid');
        break;
      case 'view:contour':
        setRenderMode('contour');
        break;
      case 'view:perspective':
        setCameraMode({ type: 'perspective' });
        break;
      case 'view:orthographic':
        setCameraMode({ type: 'orthographic' });
        break;
      case 'view:fitall':
        window.dispatchEvent(
          new CustomEvent('gfd-camera-preset', {
            detail: { position: [5, 5, 5] },
          })
        );
        break;
      case 'help:about':
        message.info('GFD - Generalized Fluid Dynamics v0.1.0');
        break;
      case 'help:docs':
        message.info('Documentation: See PROJECT_PLAN.md in the repository.');
        break;
      default:
        console.log('[Menu]', info.key);
    }
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

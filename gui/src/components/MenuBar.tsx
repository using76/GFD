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
        if (confirm('Create a new project? Unsaved changes will be lost.')) {
          window.location.reload();
        }
        break;
      case 'file:open': {
        try {
          const data = localStorage.getItem('gfd-project');
          if (data) {
            const saved = JSON.parse(data);
            const state = useAppStore.getState();
            if (saved.shapes) saved.shapes.forEach((s: any) => state.addShape(s));
            if (saved.physicsModels) state.updatePhysicsModels(saved.physicsModels);
            if (saved.material) state.updateMaterial(saved.material);
            if (saved.solverSettings) state.updateSolverSettings(saved.solverSettings);
            message.success('Project loaded from local storage.');
          } else {
            message.info('No saved project found.');
          }
        } catch { message.error('Failed to load project.'); }
        break;
      }
      case 'file:save': {
        try {
          const state = useAppStore.getState();
          const saveData = {
            shapes: state.shapes.map(s => ({ ...s, stlData: undefined })),
            physicsModels: state.physicsModels,
            material: state.material,
            solverSettings: state.solverSettings,
            boundaries: state.boundaries,
            meshConfig: state.meshConfig,
          };
          localStorage.setItem('gfd-project', JSON.stringify(saveData));
          message.success('Project saved.');
        } catch { message.error('Save failed.'); }
        break;
      }
      case 'file:saveas': {
        try {
          const state = useAppStore.getState();
          const saveData = {
            shapes: state.shapes.map(s => ({ ...s, stlData: undefined })),
            physicsModels: state.physicsModels,
            material: state.material,
            solverSettings: state.solverSettings,
            boundaries: state.boundaries,
            meshConfig: state.meshConfig,
          };
          const blob = new Blob([JSON.stringify(saveData, null, 2)], { type: 'application/json' });
          const url = URL.createObjectURL(blob);
          const a = document.createElement('a');
          a.href = url; a.download = 'gfd_project.json'; a.click();
          URL.revokeObjectURL(url);
          message.success('Project exported as JSON.');
        } catch { message.error('Export failed.'); }
        break;
      }
      case 'file:import':
        useAppStore.getState().setActiveRibbonTab('design');
        message.info('Use the Import button in the Design ribbon to import STL files.');
        break;
      case 'file:export': {
        const state = useAppStore.getState();
        if (state.fieldData.length === 0) {
          message.warning('No field data to export. Run the solver first.');
        } else {
          const lines = ['# GFD Field Data Export'];
          state.fieldData.forEach(f => {
            lines.push(`\n# ${f.name} (min=${f.min.toFixed(4)}, max=${f.max.toFixed(4)})`);
          });
          const blob = new Blob([lines.join('\n')], { type: 'text/plain' });
          const url = URL.createObjectURL(blob);
          const a = document.createElement('a');
          a.href = url; a.download = 'gfd_fields.vtk'; a.click();
          URL.revokeObjectURL(url);
          message.success('Field data exported.');
        }
        break;
      }
      case 'file:exit':
        if (confirm('Exit GFD? Unsaved changes will be lost.')) {
          window.close();
        }
        break;
      case 'edit:undo': {
        const state = useAppStore.getState();
        if (state.shapes.length > 0) {
          const last = state.shapes[state.shapes.length - 1];
          state.removeShape(last.id);
          message.info(`Undo: removed "${last.name}"`);
        } else {
          message.info('Nothing to undo');
        }
        break;
      }
      case 'edit:redo': {
        const state = useAppStore.getState();
        if (state.clipboardShape) {
          state.addShape({ ...state.clipboardShape, id: `shape-redo-${Date.now()}` });
          message.info('Redo: restored from clipboard');
        } else {
          message.info('Nothing to redo');
        }
        break;
      }
      case 'edit:preferences':
        useAppStore.getState().setActiveRibbonTab('setup');
        window.dispatchEvent(new CustomEvent('gfd-setup-section', { detail: { section: 'solver' } }));
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
        message.info('GFD - Generalized Fluid Dynamics v0.1.0 | Rust multi-physics solver');
        break;
      case 'help:docs':
        message.info('Documentation: Design > Prepare > Mesh > Setup > Calculation > Results workflow.');
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

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
      case 'file:import': {
        // Open STL file dialog
        const fileInput = document.createElement('input');
        fileInput.type = 'file';
        fileInput.accept = '.stl,.STL';
        fileInput.onchange = (ev) => {
          const file = (ev.target as HTMLInputElement).files?.[0];
          if (!file) return;
          const reader = new FileReader();
          reader.onload = (re) => {
            try {
              const buf = re.target?.result as ArrayBuffer;
              if (!buf || buf.byteLength < 84) { message.error('Invalid STL file'); return; }
              const headerBytes = new Uint8Array(buf, 0, 6);
              const headerStr = String.fromCharCode(...headerBytes);
              let verts: Float32Array;
              let fc: number;
              if (headerStr.startsWith('solid') && buf.byteLength > 84) {
                const text = new TextDecoder().decode(buf);
                const regex = /vertex\s+([-\d.eE+]+)\s+([-\d.eE+]+)\s+([-\d.eE+]+)/g;
                const coords: number[] = [];
                let m;
                while ((m = regex.exec(text)) !== null) {
                  coords.push(parseFloat(m[1]), parseFloat(m[2]), parseFloat(m[3]));
                }
                if (coords.length >= 9) {
                  verts = new Float32Array(coords);
                  fc = coords.length / 9;
                } else {
                  // Binary fallback
                  const dv = new DataView(buf);
                  fc = dv.getUint32(80, true);
                  verts = new Float32Array(fc * 9);
                  let off = 84;
                  for (let i = 0; i < fc; i++) { off += 12; for (let v = 0; v < 3; v++) { verts[i*9+v*3]=dv.getFloat32(off,true); verts[i*9+v*3+1]=dv.getFloat32(off+4,true); verts[i*9+v*3+2]=dv.getFloat32(off+8,true); off+=12; } off+=2; }
                }
              } else {
                const dv = new DataView(buf);
                fc = dv.getUint32(80, true);
                verts = new Float32Array(fc * 9);
                let off = 84;
                for (let i = 0; i < fc; i++) { off += 12; for (let v = 0; v < 3; v++) { verts[i*9+v*3]=dv.getFloat32(off,true); verts[i*9+v*3+1]=dv.getFloat32(off+4,true); verts[i*9+v*3+2]=dv.getFloat32(off+8,true); off+=12; } off+=2; }
              }
              const id = `shape-stl-${Date.now()}`;
              useAppStore.getState().addShape({
                id, name: file.name.replace(/\.stl$/i, ''), kind: 'stl',
                position: [0,0,0], rotation: [0,0,0], dimensions: {},
                stlData: { vertices: verts!, faceCount: fc! }, group: 'body',
              });
              useAppStore.getState().setActiveRibbonTab('design');
              message.success(`Imported ${file.name} (${fc!} triangles)`);
            } catch (err: any) { message.error(`Import failed: ${err.message || err}`); }
          };
          reader.readAsArrayBuffer(file);
        };
        fileInput.click();
        break;
      }
      case 'file:export': {
        const state = useAppStore.getState();
        const mesh = state.meshDisplayData;
        if (!mesh || mesh.positions.length === 0) {
          message.warning('No mesh data to export. Generate a mesh first.');
          break;
        }
        const lines: string[] = ['# vtk DataFile Version 3.0', 'GFD Export', 'ASCII', 'DATASET UNSTRUCTURED_GRID'];
        const nTriVerts = mesh.positions.length / 3;
        const nTris = nTriVerts / 3;
        lines.push(`POINTS ${nTriVerts} float`);
        for (let i = 0; i < nTriVerts; i++) {
          lines.push(`${mesh.positions[i*3].toFixed(6)} ${mesh.positions[i*3+1].toFixed(6)} ${mesh.positions[i*3+2].toFixed(6)}`);
        }
        lines.push(`CELLS ${nTris} ${nTris * 4}`);
        for (let i = 0; i < nTris; i++) lines.push(`3 ${i*3} ${i*3+1} ${i*3+2}`);
        lines.push(`CELL_TYPES ${nTris}`);
        for (let i = 0; i < nTris; i++) lines.push('5');
        if (state.fieldData.length > 0) {
          lines.push(`POINT_DATA ${nTriVerts}`);
          state.fieldData.forEach(f => {
            lines.push(`SCALARS ${f.name} float 1`, 'LOOKUP_TABLE default');
            const nV = Math.min(f.values.length, nTriVerts);
            for (let i = 0; i < nV; i++) lines.push(f.values[i].toFixed(6));
            for (let i = nV; i < nTriVerts; i++) lines.push('0.000000');
          });
        }
        const blob = new Blob([lines.join('\n')], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url; a.download = 'gfd_export.vtk'; a.click();
        URL.revokeObjectURL(url);
        message.success(`Exported VTK: ${nTris} triangles`);
        break;
      }
      case 'file:exit':
        if (confirm('Exit GFD? Unsaved changes will be lost.')) {
          window.close();
        }
        break;
      case 'edit:undo': {
        const state = useAppStore.getState();
        if (state.undoStack.length > 0) {
          state.undo();
          message.info('Undo');
        } else {
          message.info('Nothing to undo');
        }
        break;
      }
      case 'edit:redo': {
        const state = useAppStore.getState();
        if (state.redoStack.length > 0) {
          state.redo();
          message.info('Redo');
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
        break;
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

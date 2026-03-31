import React, { useState, useCallback, useRef, useEffect } from 'react';
import { ConfigProvider, theme, message } from 'antd';
import {
  UndoOutlined,
  RedoOutlined,
  SaveOutlined,
  MenuOutlined,
  FileOutlined,
  FolderOpenOutlined,
  ExportOutlined,
  SettingOutlined,
  InfoCircleOutlined,
  QuestionCircleOutlined,
} from '@ant-design/icons';
import { useAppStore } from './store/useAppStore';
import Ribbon from './components/Ribbon';
import LeftPanelStack from './components/LeftPanelStack';
import MiniToolbar from './components/MiniToolbar';
import MeasureOverlay from './components/MeasureOverlay';
import ContextMenu3D from './components/ContextMenu3D';
import StatusBar from './components/StatusBar';
import Viewport3D from './engine/Viewport3D';
import ResidualPlot from './tabs/calc/ResidualPlot';
import ConsoleOutput from './tabs/calc/ConsoleOutput';

// ============================================================
// Application Menu (Blue circle button)
// ============================================================
const AppMenu: React.FC = () => {
  const [open, setOpen] = useState(false);

  const menuItems = [
    { key: 'new', icon: <FileOutlined />, label: 'New Project', action: () => { if (confirm('Create a new project?')) window.location.reload(); } },
    { key: 'open', icon: <FolderOpenOutlined />, label: 'Open...', action: () => {
      // Use file input dialog to load .json project file
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = '.json,.gfd';
      input.onchange = (e) => {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (ev) => {
          try {
            const saved = JSON.parse(ev.target?.result as string);
            const state = useAppStore.getState();
            // Clear existing shapes
            state.shapes.forEach(s => state.removeShape(s.id));
            if (saved.shapes) saved.shapes.forEach((s: any) => state.addShape(s));
            if (saved.physicsModels) state.updatePhysicsModels(saved.physicsModels);
            if (saved.material) state.updateMaterial(saved.material);
            if (saved.solverSettings) state.updateSolverSettings(saved.solverSettings);
            if (saved.boundaries) saved.boundaries.forEach((b: any) => state.addBoundary(b));
            if (saved.meshConfig) state.updateMeshConfig(saved.meshConfig);
            message.success(`Project loaded: ${file.name}`);
          } catch { message.error('Failed to parse project file.'); }
        };
        reader.readAsText(file);
      };
      input.click();
    }},
    { key: 'restore', icon: <FolderOpenOutlined />, label: 'Restore Auto-Save', action: () => {
      try {
        const data = localStorage.getItem('gfd-autosave');
        const time = localStorage.getItem('gfd-autosave-time');
        if (!data) { message.info('No auto-save found.'); return; }
        const saved = JSON.parse(data);
        const state = useAppStore.getState();
        state.shapes.forEach(s => state.removeShape(s.id));
        if (saved.shapes) saved.shapes.forEach((s: any) => state.addShape(s));
        if (saved.physicsModels) state.updatePhysicsModels(saved.physicsModels);
        if (saved.material) state.updateMaterial(saved.material);
        if (saved.solverSettings) state.updateSolverSettings(saved.solverSettings);
        if (saved.meshConfig) state.updateMeshConfig(saved.meshConfig);
        message.success(`Auto-save restored${time ? ` (${new Date(time).toLocaleString()})` : ''}`);
      } catch { message.error('Failed to restore auto-save.'); }
    }},
    { key: 'save', icon: <SaveOutlined />, label: 'Save', action: () => {
      try {
        const state = useAppStore.getState();
        const data = {
          shapes: state.shapes.map(s => ({ ...s, stlData: undefined })),
          physicsModels: state.physicsModels,
          material: state.material,
          solverSettings: state.solverSettings,
          boundaries: state.boundaries,
          meshConfig: state.meshConfig,
        };
        localStorage.setItem('gfd-project', JSON.stringify(data));
        message.success('Project saved to local storage.');
      } catch { message.error('Failed to save project.'); }
    }},
    { key: 'saveas', icon: <SaveOutlined />, label: 'Save As...', action: () => {
      try {
        const state = useAppStore.getState();
        const data = {
          shapes: state.shapes.map(s => ({ ...s, stlData: undefined })),
          physicsModels: state.physicsModels,
          material: state.material,
          solverSettings: state.solverSettings,
          boundaries: state.boundaries,
          meshConfig: state.meshConfig,
        };
        const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'gfd_project.json';
        a.click();
        URL.revokeObjectURL(url);
        message.success('Project downloaded as gfd_project.json');
      } catch { message.error('Failed to export project.'); }
    }},
    { key: 'div1', divider: true },
    { key: 'import', icon: <FolderOpenOutlined />, label: 'Import Mesh...', action: () => {
      useAppStore.getState().setActiveRibbonTab('mesh');
      message.info('Switch to Mesh tab and click Generate to create mesh, or use Design > Import for STL files.');
    }},
    { key: 'export', icon: <ExportOutlined />, label: 'Export VTK...', action: () => {
      const state = useAppStore.getState();
      const mesh = state.meshDisplayData;
      if (!mesh || mesh.positions.length === 0) {
        message.warning('No mesh data to export. Generate a mesh first.');
        return;
      }
      // Build VTK Legacy ASCII format
      const lines: string[] = [];
      lines.push('# vtk DataFile Version 3.0');
      lines.push('GFD Export');
      lines.push('ASCII');
      lines.push('DATASET UNSTRUCTURED_GRID');

      // Extract unique vertices from triangle positions
      const nTriVerts = mesh.positions.length / 3;
      const nTris = nTriVerts / 3;
      lines.push(`POINTS ${nTriVerts} float`);
      for (let i = 0; i < nTriVerts; i++) {
        lines.push(`${mesh.positions[i*3].toFixed(6)} ${mesh.positions[i*3+1].toFixed(6)} ${mesh.positions[i*3+2].toFixed(6)}`);
      }

      // Cells: each triangle = 3 vertices
      lines.push(`CELLS ${nTris} ${nTris * 4}`);
      for (let i = 0; i < nTris; i++) {
        lines.push(`3 ${i*3} ${i*3+1} ${i*3+2}`);
      }

      // Cell types: 5 = VTK_TRIANGLE
      lines.push(`CELL_TYPES ${nTris}`);
      for (let i = 0; i < nTris; i++) lines.push('5');

      // Point data (field values)
      if (state.fieldData.length > 0) {
        lines.push(`POINT_DATA ${nTriVerts}`);
        state.fieldData.forEach(f => {
          lines.push(`SCALARS ${f.name} float 1`);
          lines.push('LOOKUP_TABLE default');
          const nVals = Math.min(f.values.length, nTriVerts);
          for (let i = 0; i < nVals; i++) {
            lines.push(f.values[i].toFixed(6));
          }
          // Pad if field values are shorter
          for (let i = nVals; i < nTriVerts; i++) {
            lines.push('0.000000');
          }
        });
      }

      const blob = new Blob([lines.join('\n')], { type: 'text/plain' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'gfd_export.vtk';
      a.click();
      URL.revokeObjectURL(url);
      message.success(`Exported VTK: ${nTris} triangles, ${state.fieldData.length} fields`);
    }},
    { key: 'exportfoam', icon: <ExportOutlined />, label: 'Export OpenFOAM...', action: () => {
      const state = useAppStore.getState();
      const ss = state.solverSettings;
      const pm = state.physicsModels;
      const mat = state.material;
      const bcs = state.boundaries;
      const mc = state.meshConfig;

      // controlDict
      const controlDict = [
        'FoamFile { version 2.0; format ascii; class dictionary; object controlDict; }',
        `application ${pm.turbulence !== 'none' ? 'simpleFoam' : 'icoFoam'};`,
        `startFrom startTime;`, `startTime 0;`,
        `stopAt endTime;`, `endTime ${ss.timeMode === 'transient' ? ss.totalTime : ss.maxIterations};`,
        `deltaT ${ss.timeMode === 'transient' ? ss.timeStepSize : 1};`,
        `writeControl timeStep;`, `writeInterval 100;`,
        `purgeWrite 0;`, `writeFormat ascii;`,
        `writePrecision 6;`, `writeCompression off;`,
        `timeFormat general;`, `timePrecision 6;`,
        `runTimeModifiable true;`,
      ].join('\n');

      // fvSchemes
      const fvSchemes = [
        'FoamFile { version 2.0; format ascii; class dictionary; object fvSchemes; }',
        'ddtSchemes { default Euler; }',
        `divSchemes { default none; div(phi,U) Gauss ${ss.momentumScheme === 'QUICK' ? 'QUICK' : 'linearUpwind grad(U)'}; }`,
        'gradSchemes { default Gauss linear; }',
        'laplacianSchemes { default Gauss linear corrected; }',
        'interpolationSchemes { default linear; }',
        'snGradSchemes { default corrected; }',
      ].join('\n');

      // fvSolution
      const fvSolution = [
        'FoamFile { version 2.0; format ascii; class dictionary; object fvSolution; }',
        `solvers { U { solver smoothSolver; smoother symGaussSeidel; tolerance ${ss.tolerance}; relTol 0.1; } p { solver GAMG; tolerance ${ss.tolerance}; relTol 0.01; smoother GaussSeidel; } }`,
        `${ss.method} { nNonOrthogonalCorrectors 0; pRefCell 0; pRefValue 0; }`,
        `relaxationFactors { fields { p ${ss.relaxPressure}; } equations { U ${ss.relaxVelocity}; } }`,
      ].join('\n');

      // transportProperties
      const transport = [
        'FoamFile { version 2.0; format ascii; class dictionary; object transportProperties; }',
        `transportModel Newtonian;`,
        `nu nu [0 2 -1 0 0 0 0] ${(mat.viscosity / mat.density).toExponential(6)};`,
      ].join('\n');

      // Boundary conditions (0/U, 0/p)
      const bcU = bcs.map(b => {
        if (b.type === 'inlet') return `  ${b.name.replace(/[^a-zA-Z0-9_]/g, '_')} { type fixedValue; value uniform (${b.velocity.join(' ')}); }`;
        if (b.type === 'outlet') return `  ${b.name.replace(/[^a-zA-Z0-9_]/g, '_')} { type zeroGradient; }`;
        if (b.type === 'wall') return `  ${b.name.replace(/[^a-zA-Z0-9_]/g, '_')} { type noSlip; }`;
        return `  ${b.name.replace(/[^a-zA-Z0-9_]/g, '_')} { type symmetryPlane; }`;
      }).join('\n');
      const uFile = `FoamFile { version 2.0; format ascii; class volVectorField; object U; }\ndimensions [0 1 -1 0 0 0 0];\ninternalField uniform (0 0 0);\nboundaryField {\n${bcU}\n}`;

      const bcP = bcs.map(b => {
        if (b.type === 'outlet') return `  ${b.name.replace(/[^a-zA-Z0-9_]/g, '_')} { type fixedValue; value uniform ${b.pressure}; }`;
        return `  ${b.name.replace(/[^a-zA-Z0-9_]/g, '_')} { type zeroGradient; }`;
      }).join('\n');
      const pFile = `FoamFile { version 2.0; format ascii; class volScalarField; object p; }\ndimensions [0 2 -2 0 0 0 0];\ninternalField uniform 0;\nboundaryField {\n${bcP}\n}`;

      // Bundle as a single file with markers
      const bundle = [
        '=== system/controlDict ===', controlDict,
        '=== system/fvSchemes ===', fvSchemes,
        '=== system/fvSolution ===', fvSolution,
        '=== constant/transportProperties ===', transport,
        '=== 0/U ===', uFile,
        '=== 0/p ===', pFile,
        `=== README ===`,
        `OpenFOAM case exported from GFD GUI`,
        `Mesh: ${mc.type} ${mc.globalSize}m, Solver: ${ss.method}`,
        `Split this file at "===" markers to create the case directory structure.`,
      ].join('\n\n');

      const blob = new Blob([bundle], { type: 'text/plain' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url; a.download = 'gfd_openfoam_case.txt'; a.click();
      URL.revokeObjectURL(url);
      message.success('OpenFOAM case exported (controlDict, fvSchemes, fvSolution, BCs)');
    }},
    { key: 'exportstl', icon: <ExportOutlined />, label: 'Export STL...', action: async () => {
      const state = useAppStore.getState();
      const sel = state.selectedShapeId;
      const shape = sel ? state.shapes.find(s => s.id === sel) : null;
      if (!shape) { message.warning('Select a shape to export as STL.'); return; }
      // Generate binary STL from shape geometry
      if (shape.kind === 'stl' && shape.stlData) {
        // Re-export existing STL data
        const fc = shape.stlData.faceCount;
        const verts = shape.stlData.vertices;
        const buf = new ArrayBuffer(84 + fc * 50);
        const dv = new DataView(buf);
        // Header (80 bytes)
        const header = `GFD Export: ${shape.name}`;
        for (let i = 0; i < Math.min(80, header.length); i++) dv.setUint8(i, header.charCodeAt(i));
        dv.setUint32(80, fc, true);
        let off = 84;
        for (let i = 0; i < fc; i++) {
          // Normal (0,0,0)
          dv.setFloat32(off, 0, true); dv.setFloat32(off+4, 0, true); dv.setFloat32(off+8, 0, true); off += 12;
          for (let v = 0; v < 3; v++) {
            dv.setFloat32(off, verts[i*9+v*3], true);
            dv.setFloat32(off+4, verts[i*9+v*3+1], true);
            dv.setFloat32(off+8, verts[i*9+v*3+2], true);
            off += 12;
          }
          dv.setUint16(off, 0, true); off += 2;
        }
        const blob = new Blob([buf], { type: 'application/octet-stream' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url; a.download = `${shape.name}.stl`; a.click();
        URL.revokeObjectURL(url);
        message.success(`Exported ${shape.name}.stl (${fc} triangles)`);
      } else {
        // Generate triangulated surface from primitive using Three.js geometry
        const THREE = await import('three');
        let geom: InstanceType<typeof THREE.BufferGeometry>;
        const d = shape.dimensions;
        switch (shape.kind) {
          case 'box': case 'enclosure':
            geom = new THREE.BoxGeometry(d.width ?? 1, d.height ?? 1, d.depth ?? 1); break;
          case 'sphere':
            geom = new THREE.SphereGeometry(d.radius ?? 0.5, 32, 32); break;
          case 'cylinder':
            geom = new THREE.CylinderGeometry(d.radius ?? 0.3, d.radius ?? 0.3, d.height ?? 1, 32); break;
          case 'cone':
            geom = new THREE.ConeGeometry(d.radius ?? 0.4, d.height ?? 1, 32); break;
          case 'torus':
            geom = new THREE.TorusGeometry(d.majorRadius ?? 0.5, d.minorRadius ?? 0.15, 16, 48); break;
          case 'pipe':
            geom = new THREE.CylinderGeometry(d.outerRadius ?? 0.4, d.outerRadius ?? 0.4, d.height ?? 1.5, 32); break;
          default:
            geom = new THREE.BoxGeometry(1, 1, 1);
        }
        // Extract non-indexed triangles
        const nonIndexed = geom.index ? geom.toNonIndexed() : geom;
        const posAttr = nonIndexed.getAttribute('position') as InstanceType<typeof THREE.BufferAttribute>;
        const nVerts = posAttr.count;
        const fc = nVerts / 3;
        const buf = new ArrayBuffer(84 + fc * 50);
        const dv = new DataView(buf);
        const header = `GFD Export: ${shape.name}`;
        for (let i = 0; i < Math.min(80, header.length); i++) dv.setUint8(i, header.charCodeAt(i));
        dv.setUint32(80, fc, true);
        let off = 84;
        for (let i = 0; i < fc; i++) {
          // Compute normal
          const i0 = i * 3, i1 = i * 3 + 1, i2 = i * 3 + 2;
          const ax = posAttr.getX(i0) + shape.position[0], ay = posAttr.getY(i0) + shape.position[1], az = posAttr.getZ(i0) + shape.position[2];
          const bx = posAttr.getX(i1) + shape.position[0], by = posAttr.getY(i1) + shape.position[1], bz = posAttr.getZ(i1) + shape.position[2];
          const cx = posAttr.getX(i2) + shape.position[0], cy = posAttr.getY(i2) + shape.position[1], cz = posAttr.getZ(i2) + shape.position[2];
          const e1x = bx-ax, e1y = by-ay, e1z = bz-az;
          const e2x = cx-ax, e2y = cy-ay, e2z = cz-az;
          const nx = e1y*e2z - e1z*e2y, ny = e1z*e2x - e1x*e2z, nz = e1x*e2y - e1y*e2x;
          const nl = Math.sqrt(nx*nx + ny*ny + nz*nz) || 1;
          dv.setFloat32(off, nx/nl, true); dv.setFloat32(off+4, ny/nl, true); dv.setFloat32(off+8, nz/nl, true); off += 12;
          dv.setFloat32(off, ax, true); dv.setFloat32(off+4, ay, true); dv.setFloat32(off+8, az, true); off += 12;
          dv.setFloat32(off, bx, true); dv.setFloat32(off+4, by, true); dv.setFloat32(off+8, bz, true); off += 12;
          dv.setFloat32(off, cx, true); dv.setFloat32(off+4, cy, true); dv.setFloat32(off+8, cz, true); off += 12;
          dv.setUint16(off, 0, true); off += 2;
        }
        geom.dispose();
        nonIndexed.dispose();
        const blob = new Blob([buf], { type: 'application/octet-stream' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url; a.download = `${shape.name}.stl`; a.click();
        URL.revokeObjectURL(url);
        message.success(`Exported ${shape.name}.stl (${fc} triangles)`);
      }
    }},
    { key: 'exportgmsh', icon: <ExportOutlined />, label: 'Export Gmsh...', action: () => {
      const state = useAppStore.getState();
      const mesh = state.meshDisplayData;
      if (!mesh || mesh.positions.length === 0) { message.warning('No mesh to export.'); return; }
      const nTriVerts = mesh.positions.length / 3;
      const nTris = nTriVerts / 3;
      const lines: string[] = [];
      lines.push('$MeshFormat', '2.2 0 8', '$EndMeshFormat');
      // Nodes
      lines.push('$Nodes', String(nTriVerts));
      for (let i = 0; i < nTriVerts; i++) {
        lines.push(`${i+1} ${mesh.positions[i*3].toFixed(8)} ${mesh.positions[i*3+1].toFixed(8)} ${mesh.positions[i*3+2].toFixed(8)}`);
      }
      lines.push('$EndNodes');
      // Elements (triangles, type 2)
      lines.push('$Elements', String(nTris));
      for (let i = 0; i < nTris; i++) {
        lines.push(`${i+1} 2 2 1 1 ${i*3+1} ${i*3+2} ${i*3+3}`);
      }
      lines.push('$EndElements');
      const blob = new Blob([lines.join('\n')], { type: 'text/plain' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url; a.download = 'gfd_mesh.msh'; a.click();
      URL.revokeObjectURL(url);
      message.success(`Exported Gmsh: ${nTris} triangles, ${nTriVerts} nodes`);
    }},
    { key: 'div2', divider: true },
    { key: 'settings', icon: <SettingOutlined />, label: 'Settings', action: () => {
      useAppStore.getState().setActiveRibbonTab('setup');
      window.dispatchEvent(new CustomEvent('gfd-setup-section', { detail: { section: 'solver' } }));
    }},
    { key: 'about', icon: <InfoCircleOutlined />, label: 'About GFD', action: () => {
      const state = useAppStore.getState();
      const shapeCount = state.shapes.length;
      const meshCells = state.meshDisplayData?.cellCount ?? 0;
      message.info({
        content: `GFD — Generalized Fluid Dynamics v0.1.0\n` +
          `Rust solver: 262 files, 63K lines, 19 crates, 805 tests\n` +
          `GUI: 50 files, 14K lines (React + Three.js)\n` +
          `Current: ${shapeCount} shapes, ${meshCells} mesh cells\n` +
          `License: Modified MIT`,
        duration: 8,
      });
    }},
  ];

  return (
    <div style={{ position: 'relative' }}>
      <div
        onClick={() => setOpen(!open)}
        style={{
          width: 30,
          height: 30,
          borderRadius: '50%',
          background: 'linear-gradient(135deg, #2060cc, #1040aa)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          cursor: 'pointer',
          boxShadow: '0 1px 4px rgba(0,0,0,0.4)',
          flexShrink: 0,
        }}
      >
        <MenuOutlined style={{ color: '#fff', fontSize: 12 }} />
      </div>

      {open && (
        <>
          {/* Backdrop */}
          <div
            onClick={() => setOpen(false)}
            style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, zIndex: 999 }}
          />
          {/* Menu dropdown */}
          <div style={{
            position: 'absolute',
            top: 34,
            left: 0,
            width: 220,
            background: '#1a1a2e',
            border: '1px solid #303050',
            borderRadius: 6,
            padding: '4px 0',
            zIndex: 1000,
            boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
          }}>
            {menuItems.map((item) => {
              if ('divider' in item && item.divider) {
                return <div key={item.key} style={{ height: 1, background: '#303050', margin: '4px 8px' }} />;
              }
              const mi = item as { key: string; icon?: React.ReactNode; label?: string; action?: () => void };
              return (
                <div
                  key={mi.key}
                  onClick={() => {
                    setOpen(false);
                    mi.action?.();
                  }}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 8,
                    padding: '6px 12px',
                    cursor: 'pointer',
                    color: '#bbc',
                    fontSize: 12,
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.background = '#252540'; }}
                  onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; }}
                >
                  <span style={{ fontSize: 13, width: 16, textAlign: 'center', color: '#889' }}>
                    {mi.icon}
                  </span>
                  {mi.label}
                </div>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
};

// ============================================================
// Quick Access Toolbar
// ============================================================
const QuickAccess: React.FC = () => (
  <div style={{ display: 'flex', alignItems: 'center', gap: 2 }}>
    {[
      { icon: <UndoOutlined />, tip: 'Undo', action: () => {
        const state = useAppStore.getState();
        if (state.undoStack.length > 0) {
          state.undo();
          message.info('Undo');
        } else {
          message.info('Nothing to undo');
        }
      }},
      { icon: <RedoOutlined />, tip: 'Redo', action: () => {
        const state = useAppStore.getState();
        if (state.redoStack.length > 0) {
          state.redo();
          message.info('Redo');
        } else {
          message.info('Nothing to redo');
        }
      }},
      { icon: <SaveOutlined />, tip: 'Save', action: () => {
        try {
          const state = useAppStore.getState();
          const data = {
            shapes: state.shapes.map(s => ({ ...s, stlData: undefined })),
            physicsModels: state.physicsModels,
            material: state.material,
            solverSettings: state.solverSettings,
            boundaries: state.boundaries,
            meshConfig: state.meshConfig,
          };
          localStorage.setItem('gfd-project', JSON.stringify(data));
          message.success('Project saved.');
        } catch { message.error('Save failed.'); }
      }},
    ].map((btn, i) => (
      <div
        key={i}
        onClick={btn.action}
        title={btn.tip}
        style={{
          width: 24,
          height: 24,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          borderRadius: 3,
          cursor: 'pointer',
          color: '#889',
          fontSize: 13,
        }}
        onMouseEnter={(e) => { e.currentTarget.style.background = '#252540'; e.currentTarget.style.color = '#bbc'; }}
        onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#889'; }}
      >
        {btn.icon}
      </div>
    ))}
  </div>
);

// ============================================================
// Resizable Left Panel Wrapper
// ============================================================
const LEFT_MIN = 200;
const LEFT_MAX = 500;
const LEFT_DEFAULT = 270;

const ResizableLeftPanel: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [width, setWidth] = useState(LEFT_DEFAULT);
  const dragging = useRef(false);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    dragging.current = true;
    const startX = e.clientX;
    const startW = width;

    const onMove = (ev: MouseEvent) => {
      if (!dragging.current) return;
      const newW = Math.max(LEFT_MIN, Math.min(LEFT_MAX, startW + ev.clientX - startX));
      setWidth(newW);
    };
    const onUp = () => {
      dragging.current = false;
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }, [width]);

  return (
    <div style={{ display: 'flex', flexShrink: 0, height: '100%' }}>
      <div style={{ width, minWidth: LEFT_MIN, height: '100%', overflow: 'hidden', borderRight: '1px solid #252540' }}>
        {children}
      </div>
      <div
        onMouseDown={onMouseDown}
        style={{ width: 4, cursor: 'col-resize', background: '#1a1a30', flexShrink: 0 }}
        onMouseEnter={(e) => { e.currentTarget.style.background = '#303060'; }}
        onMouseLeave={(e) => { e.currentTarget.style.background = '#1a1a30'; }}
      />
    </div>
  );
};

// ============================================================
// Center Content (viewport or calc views)
// ============================================================
const CenterContent: React.FC = () => {
  const activeRibbonTab = useAppStore((s) => s.activeRibbonTab);

  // For Calculation tab, show residual plot or console instead of viewport
  if (activeRibbonTab === 'calc') {
    return (
      <div style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column' }}>
        <div style={{ flex: 1, minHeight: 0 }}>
          <ResidualPlot />
        </div>
        <div style={{ height: 200, borderTop: '1px solid #252540', flexShrink: 0 }}>
          <ConsoleOutput />
        </div>
      </div>
    );
  }

  // For all other tabs, show 3D viewport with mini toolbar, measure overlay, and context menu support
  return (
    <div
      style={{ width: '100%', height: '100%', position: 'relative' }}
      onContextMenu={(e) => {
        e.preventDefault();
        const selectedShapeId = useAppStore.getState().selectedShapeId;
        useAppStore.getState().setContextMenu({
          x: e.clientX,
          y: e.clientY,
          shapeId: selectedShapeId,
        });
      }}
    >
      <Viewport3D />
      <MiniToolbar />
      <MeasureOverlay />
    </div>
  );
};

// ============================================================
// View Presets (for keyboard shortcuts)
// ============================================================
const VIEW_PRESET_POSITIONS: Record<string, [number, number, number]> = {
  '1': [0, 0, 8],   // Front
  '2': [0, 0, -8],  // Back
  '3': [0, 8, 0.01],  // Top
  '4': [0, -8, 0.01], // Bottom
  '5': [-8, 0, 0],  // Left
  '6': [8, 0, 0],   // Right
  '0': [5, 5, 5],   // Isometric
};

let pasteCounter = 300;

// ============================================================
// Keyboard Shortcuts Hook
// ============================================================
function useKeyboardShortcuts() {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Ignore when typing in input fields
      const target = e.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT' || target.isContentEditable) {
        return;
      }

      const store = useAppStore.getState();

      // Ctrl combinations
      if (e.ctrlKey || e.metaKey) {
        switch (e.key.toLowerCase()) {
          case 'z':
            e.preventDefault();
            if (e.shiftKey) {
              if (store.redoStack.length > 0) {
                store.redo();
                message.info('Redo');
              }
            } else {
              if (store.undoStack.length > 0) {
                store.undo();
                message.info('Undo');
              }
            }
            return;
          case 'p':
            e.preventDefault();
            window.dispatchEvent(new CustomEvent('gfd-screenshot'));
            message.success('Screenshot saved');
            return;
          case 's':
            e.preventDefault();
            try {
              const data = {
                shapes: store.shapes.map(s => ({ ...s, stlData: undefined })),
                physicsModels: store.physicsModels,
                material: store.material,
                solverSettings: store.solverSettings,
                boundaries: store.boundaries,
                meshConfig: store.meshConfig,
              };
              localStorage.setItem('gfd-project', JSON.stringify(data));
              message.success('Project saved');
            } catch { message.error('Save failed'); }
            return;
          case 'x':
            e.preventDefault();
            if (store.selectedShapeId) {
              const shape = store.shapes.find(s => s.id === store.selectedShapeId);
              if (shape) {
                store.setClipboardShape({ ...shape });
                store.setClipboardShapeId(store.selectedShapeId);
                store.removeShape(store.selectedShapeId);
                message.info(`Cut "${shape.name}"`);
              }
            }
            return;
          case 'c':
            e.preventDefault();
            if (store.selectedShapeId) {
              const shape = store.shapes.find(s => s.id === store.selectedShapeId);
              if (shape) store.setClipboardShape({ ...shape });
              store.setClipboardShapeId(store.selectedShapeId);
              message.info('Shape copied');
            }
            return;
          case 'v': {
            e.preventDefault();
            // Prefer full clipboardShape (works even after source is deleted via cut)
            const source = store.clipboardShape ?? store.shapes.find(s => s.id === store.clipboardShapeId);
            if (source) {
              const id = `shape-${pasteCounter++}`;
              store.addShape({
                ...source,
                id,
                name: `${source.name}-paste`,
                position: [source.position[0] + 0.5, source.position[1], source.position[2] + 0.5],
                stlData: source.stlData,
              });
              store.selectShape(id);
              message.success('Shape pasted');
            }
            return;
          }
          case 'y':
            e.preventDefault();
            if (store.redoStack.length > 0) {
              store.redo();
              message.info('Redo');
            }
            return;
        }
        return;
      }

      // Function keys
      if (e.key === 'F11') {
        e.preventDefault();
        if (!document.fullscreenElement) {
          document.documentElement.requestFullscreen().catch(() => {});
        } else {
          document.exitFullscreen().catch(() => {});
        }
        return;
      }

      // Single key shortcuts (no modifiers)
      switch (e.key) {
        case 's':
        case 'S':
          e.preventDefault();
          store.setActiveTool('select');
          return;
        case 'p':
        case 'P':
          e.preventDefault();
          store.setActiveTool('pull');
          return;
        case 'm':
        case 'M':
          e.preventDefault();
          store.setActiveTool('move');
          return;
        case 'r':
        case 'R':
          e.preventDefault();
          // Toggle transform mode: translate → rotate → scale → translate
          { const modes: Array<'translate' | 'rotate' | 'scale'> = ['translate', 'rotate', 'scale'];
            const cur = store.transformMode;
            const next = modes[(modes.indexOf(cur) + 1) % modes.length];
            store.setTransformMode(next);
            message.info(`Transform mode: ${next}`);
          }
          return;
        case 'f':
        case 'F':
          e.preventDefault();
          store.setActiveTool('fill');
          return;
        case 'Delete':
        case 'Backspace':
          if (store.selectedShapeId) {
            const name = store.shapes.find(s => s.id === store.selectedShapeId)?.name ?? '';
            store.removeShape(store.selectedShapeId);
            message.info(`Deleted ${name}`);
          }
          return;
        case 'h':
        case 'H':
          e.preventDefault();
          window.dispatchEvent(new CustomEvent('gfd-camera-preset', { detail: { position: [5, 5, 5] } }));
          return;
        case 'Escape':
          store.selectShape(null);
          store.setMeasureMode(null);
          store.setContextMenu(null);
          store.setCadMode('select');
          return;
        case '?':
          e.preventDefault();
          window.dispatchEvent(new CustomEvent('gfd-show-shortcuts'));
          return;
      }

      // Number keys for view presets
      if (e.key >= '0' && e.key <= '6') {
        const pos = VIEW_PRESET_POSITIONS[e.key];
        if (pos) {
          e.preventDefault();
          window.dispatchEvent(new CustomEvent('gfd-camera-preset', { detail: { position: pos } }));
        }
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);
}

// ============================================================
// Auto-save (every 5 minutes)
// ============================================================
function useAutoSave() {
  useEffect(() => {
    const interval = setInterval(() => {
      try {
        const state = useAppStore.getState();
        if (state.shapes.length === 0) return; // Nothing to save
        const data = {
          shapes: state.shapes.map(s => ({ ...s, stlData: undefined })),
          physicsModels: state.physicsModels,
          material: state.material,
          solverSettings: state.solverSettings,
          boundaries: state.boundaries,
          meshConfig: state.meshConfig,
        };
        localStorage.setItem('gfd-autosave', JSON.stringify(data));
        localStorage.setItem('gfd-autosave-time', new Date().toISOString());
      } catch { /* ignore save errors */ }
    }, 5 * 60 * 1000); // 5 minutes
    return () => clearInterval(interval);
  }, []);
}

// ============================================================
// Keyboard Shortcuts Overlay
// ============================================================
function ShortcutsOverlay() {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const handler = () => setVisible(true);
    window.addEventListener('gfd-show-shortcuts', handler);
    return () => window.removeEventListener('gfd-show-shortcuts', handler);
  }, []);

  if (!visible) return null;

  const shortcuts = [
    ['Ctrl+S', 'Save project'],
    ['Ctrl+Z', 'Undo'],
    ['Ctrl+Shift+Z / Ctrl+Y', 'Redo'],
    ['Ctrl+C', 'Copy shape'],
    ['Ctrl+X', 'Cut shape'],
    ['Ctrl+V', 'Paste shape'],
    ['Ctrl+P', 'Screenshot'],
    ['S', 'Select tool'],
    ['P', 'Pull tool'],
    ['M', 'Move tool'],
    ['F', 'Fill tool'],
    ['R', 'Cycle transform: Translate/Rotate/Scale'],
    ['H', 'Home camera'],
    ['Delete', 'Delete selected'],
    ['Escape', 'Deselect / Cancel'],
    ['F11', 'Fullscreen toggle'],
    ['0-6', 'Camera presets (Iso/Front/Back/Top/Bottom/Left/Right)'],
    ['?', 'Show this help'],
    ['Double-click', 'Place probe point (when field data available)'],
  ];

  return (
    <div
      onClick={() => setVisible(false)}
      style={{
        position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.7)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        zIndex: 9999,
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: '#1a1a2e', border: '1px solid #303050', borderRadius: 12,
          padding: '24px 32px', maxWidth: 500, width: '90%',
          boxShadow: '0 8px 32px rgba(0,0,0,0.6)',
        }}
      >
        <div style={{ fontSize: 16, fontWeight: 700, color: '#ccd', marginBottom: 16, borderBottom: '1px solid #303050', paddingBottom: 8 }}>
          Keyboard Shortcuts
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '6px 16px' }}>
          {shortcuts.map(([key, desc]) => (
            <React.Fragment key={key}>
              <span style={{ fontSize: 12, color: '#4096ff', fontFamily: 'monospace', fontWeight: 600 }}>{key}</span>
              <span style={{ fontSize: 12, color: '#aab' }}>{desc}</span>
            </React.Fragment>
          ))}
        </div>
        <div style={{ marginTop: 16, textAlign: 'center', color: '#556', fontSize: 11 }}>
          Press Escape or click outside to close
        </div>
      </div>
    </div>
  );
}

export default function App() {
  const TITLE_BAR_H = 36;
  const STATUS_BAR_H = 28;

  useKeyboardShortcuts();
  useAutoSave();

  return (
    <ConfigProvider
      theme={{
        algorithm: theme.darkAlgorithm,
        token: { colorPrimary: '#4096ff', borderRadius: 4, fontSize: 12 },
      }}
    >
      <style>{`
        html, body, #root { margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #0a0a18; }
        ::-webkit-scrollbar { width: 6px; height: 6px; }
        ::-webkit-scrollbar-track { background: #111122; }
        ::-webkit-scrollbar-thumb { background: #333355; border-radius: 3px; }
        ::-webkit-scrollbar-thumb:hover { background: #444466; }
        .ant-tree { background: transparent !important; }
        .ant-tree .ant-tree-node-content-wrapper { color: #aab !important; font-size: 12px !important; }
        .ant-tree .ant-tree-node-content-wrapper:hover { background: #1a1a3a !important; }
        .ant-tree .ant-tree-node-content-wrapper.ant-tree-node-selected { background: #2a2a5a !important; color: #fff !important; }
        .ant-form-item-label > label { color: #889 !important; font-size: 11px !important; }
        .ant-select-selector { background: #1a1a30 !important; border-color: #303050 !important; }
        .ant-input-number, .ant-input { background: #1a1a30 !important; border-color: #303050 !important; }
      `}</style>

      <div style={{ width: '100vw', height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>

        {/* ============ Title Bar ============ */}
        <div style={{
          height: TITLE_BAR_H,
          background: '#12122a',
          borderBottom: '1px solid #252540',
          display: 'flex',
          alignItems: 'center',
          padding: '0 10px',
          gap: 10,
          flexShrink: 0,
        }}>
          <AppMenu />
          <QuickAccess />
          <div style={{ flex: 1 }} />
          <span style={{ fontSize: 12, color: '#556', fontWeight: 500, letterSpacing: 0.5 }}>
            GFD - Generalized Fluid Dynamics
          </span>
          <div style={{ flex: 1 }} />
          <QuestionCircleOutlined style={{ color: '#445', cursor: 'pointer', fontSize: 14 }} onClick={() => message.info('GFD GUI: Design > Prepare > Mesh > Setup > Calculation > Results. Keyboard: Ctrl+Z undo, Ctrl+C copy, Del delete.')} />
        </div>

        {/* ============ Ribbon ============ */}
        <Ribbon />

        {/* ============ Main Content: Left Panel + Center ============ */}
        <div style={{ flex: 1, display: 'flex', overflow: 'hidden', minHeight: 0 }}>

          {/* Left Panel Stack */}
          <ResizableLeftPanel>
            <LeftPanelStack />
          </ResizableLeftPanel>

          {/* Center: Viewport / Calc content */}
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', minWidth: 200 }}>
            <div style={{ flex: 1, minHeight: 0, overflow: 'hidden' }}>
              <CenterContent />
            </div>
          </div>
        </div>

        {/* ============ Status Bar ============ */}
        <div style={{
          height: STATUS_BAR_H,
          background: '#12122a',
          borderTop: '1px solid #252540',
          flexShrink: 0,
        }}>
          <StatusBar />
        </div>
      </div>

      {/* ============ Context Menu (rendered at top level for z-index) ============ */}
      <ContextMenu3D />

      {/* ============ Keyboard Shortcuts Overlay ============ */}
      <ShortcutsOverlay />
    </ConfigProvider>
  );
}

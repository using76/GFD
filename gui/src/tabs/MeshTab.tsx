import React, { useMemo } from 'react';
import SplitLayout from '../components/SplitLayout';
import MeshZoneTree from './mesh/MeshZoneTree';
import MeshSettings from './mesh/MeshSettings';
import QualityPanel from './mesh/QualityPanel';
import BoundaryEditor from './mesh/BoundaryEditor';
import { useAppStore } from '../store/useAppStore';

interface MeshTabProps {
  viewport: React.ReactNode;
}

const MeshTab: React.FC<MeshTabProps> = ({ viewport }) => {
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const editingSurfaceId = useAppStore((s) => s.editingSurfaceId);
  const selectedMeshSurfaceId = useAppStore((s) => s.selectedMeshSurfaceId);

  // Show BoundaryEditor when a surface is selected for editing;
  // otherwise show MeshSettings + QualityPanel.
  const showBoundaryEditor = !!(editingSurfaceId || selectedMeshSurfaceId);

  const leftPanel = useMemo(() => (
    <div style={{ height: '100%', overflow: 'auto' }}>
      <MeshZoneTree />
    </div>
  ), []);

  const rightPanel = useMemo(() => (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {showBoundaryEditor ? (
        <div style={{ flex: 1, overflow: 'auto' }}>
          <BoundaryEditor />
        </div>
      ) : (
        <>
          <div style={{ flex: 1, overflow: 'auto' }}>
            <MeshSettings />
          </div>
          {meshGenerated && (
            <div style={{ borderTop: '1px solid #303030', overflow: 'auto', maxHeight: '50%' }}>
              <QualityPanel />
            </div>
          )}
        </>
      )}
    </div>
  ), [showBoundaryEditor, meshGenerated]);

  return (
    <SplitLayout left={leftPanel} center={viewport} right={rightPanel} />
  );
};

export default MeshTab;

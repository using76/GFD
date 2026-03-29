import React, { useCallback, useMemo, useState } from 'react';
import { Dropdown } from 'antd';
import type { MenuProps } from 'antd';
import {
  FolderOutlined,
  AppstoreOutlined,
  BorderInnerOutlined,
  EyeOutlined,
  EyeInvisibleOutlined,
} from '@ant-design/icons';
import { useAppStore, BOUNDARY_COLORS } from '../../store/useAppStore';
import type { MeshSurfaceBoundaryType } from '../../store/useAppStore';

/** Color dot for boundary type indicators */
const ColorDot: React.FC<{ color: string; size?: number }> = ({ color, size = 8 }) => (
  <span
    style={{
      display: 'inline-block',
      width: size,
      height: size,
      borderRadius: '50%',
      background: color,
      flexShrink: 0,
    }}
  />
);

/** Icons for boundary types */
const boundaryTypeLabels: Record<MeshSurfaceBoundaryType, string> = {
  inlet: 'Inlet',
  outlet: 'Outlet',
  wall: 'Wall',
  symmetry: 'Symmetry',
  periodic: 'Periodic',
  open: 'Open',
  none: 'Unassigned',
};

const MeshZoneTree: React.FC = () => {
  const meshVolumes = useAppStore((s) => s.meshVolumes);
  const meshSurfaces = useAppStore((s) => s.meshSurfaces);
  const selectedMeshVolumeId = useAppStore((s) => s.selectedMeshVolumeId);
  const selectedMeshSurfaceId = useAppStore((s) => s.selectedMeshSurfaceId);
  const selectMeshVolume = useAppStore((s) => s.selectMeshVolume);
  const selectMeshSurface = useAppStore((s) => s.selectMeshSurface);
  const setEditingSurface = useAppStore((s) => s.setEditingSurface);
  const addBoundarySurface = useAppStore((s) => s.addBoundarySurface);
  const setMeshVolumes = useAppStore((s) => s.setMeshVolumes);
  const fluidExtracted = useAppStore((s) => s.fluidExtracted);

  const [expandedVolumes, setExpandedVolumes] = useState(true);
  const [expandedSurfaces, setExpandedSurfaces] = useState(true);

  const handleVolumeClick = useCallback((id: string) => {
    selectMeshVolume(id);
  }, [selectMeshVolume]);

  const handleVolumeToggleVisible = useCallback((id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const state = useAppStore.getState();
    const updated = state.meshVolumes.map((v) =>
      v.id === id ? { ...v, visible: !v.visible } : v
    );
    setMeshVolumes(updated);
  }, [setMeshVolumes]);

  const handleSurfaceClick = useCallback((id: string) => {
    selectMeshSurface(id);
    setEditingSurface(id);
  }, [selectMeshSurface, setEditingSurface]);

  // Context menu for adding boundaries
  const surfacesContextMenuItems: MenuProps['items'] = useMemo(() => {
    const types: MeshSurfaceBoundaryType[] = ['inlet', 'outlet', 'wall', 'symmetry', 'periodic', 'open'];
    return types.map((type) => ({
      key: type,
      icon: <ColorDot color={BOUNDARY_COLORS[type]} size={10} />,
      label: `Add ${boundaryTypeLabels[type]}`,
      onClick: () => addBoundarySurface(type),
    }));
  }, [addBoundarySurface]);

  // Group surfaces: boundaries (assigned) vs faces (unassigned)
  const assignedSurfaces = useMemo(
    () => meshSurfaces.filter((s) => s.boundaryType !== 'none'),
    [meshSurfaces]
  );
  const unassignedCount = useMemo(
    () => meshSurfaces.filter((s) => s.boundaryType === 'none').length,
    [meshSurfaces]
  );

  if (!fluidExtracted && meshVolumes.length === 0) {
    return (
      <div style={{ padding: 12, color: '#666', fontSize: 12 }}>
        <div style={{ padding: '8px 0', fontWeight: 600, borderBottom: '1px solid #303030', marginBottom: 8 }}>
          Mesh Zones
        </div>
        <div style={{ color: '#555', fontStyle: 'italic', padding: '16px 4px' }}>
          Extract fluid volume in the Prepare tab to populate mesh zones.
        </div>
      </div>
    );
  }

  const itemStyle = (isSelected: boolean): React.CSSProperties => ({
    display: 'flex',
    alignItems: 'center',
    gap: 6,
    padding: '4px 8px 4px 24px',
    fontSize: 12,
    cursor: 'pointer',
    background: isSelected ? '#1a2a4a' : 'transparent',
    color: isSelected ? '#4096ff' : '#bbc',
    borderLeft: isSelected ? '2px solid #4096ff' : '2px solid transparent',
    userSelect: 'none' as const,
  });

  const groupHeaderStyle: React.CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: 6,
    padding: '6px 8px',
    fontSize: 12,
    fontWeight: 600,
    color: '#aab',
    cursor: 'pointer',
    userSelect: 'none',
  };

  return (
    <div style={{ padding: 0, fontSize: 12 }}>
      <div
        style={{
          padding: '8px 12px',
          fontWeight: 600,
          borderBottom: '1px solid #303030',
          color: '#ccd',
        }}
      >
        Mesh Zones
      </div>

      {/* Volumes group */}
      <div>
        <div
          style={groupHeaderStyle}
          onClick={() => setExpandedVolumes(!expandedVolumes)}
        >
          <FolderOutlined style={{ fontSize: 11 }} />
          <AppstoreOutlined style={{ fontSize: 11, color: '#4488ff' }} />
          <span>Volumes</span>
          <span style={{ color: '#556', fontSize: 10, marginLeft: 'auto' }}>
            {meshVolumes.length}
          </span>
        </div>
        {expandedVolumes && meshVolumes.map((vol) => (
          <div
            key={vol.id}
            style={itemStyle(selectedMeshVolumeId === vol.id)}
            onClick={() => handleVolumeClick(vol.id)}
            onMouseEnter={(e) => {
              if (selectedMeshVolumeId !== vol.id) {
                e.currentTarget.style.background = '#161630';
              }
            }}
            onMouseLeave={(e) => {
              if (selectedMeshVolumeId !== vol.id) {
                e.currentTarget.style.background = 'transparent';
              }
            }}
          >
            <ColorDot color={vol.color} size={10} />
            <span style={{ flex: 1 }}>{vol.name}</span>
            <span
              onClick={(e) => handleVolumeToggleVisible(vol.id, e)}
              style={{ color: vol.visible ? '#aab' : '#444', cursor: 'pointer', fontSize: 13 }}
              title={vol.visible ? 'Hide' : 'Show'}
            >
              {vol.visible ? <EyeOutlined /> : <EyeInvisibleOutlined />}
            </span>
            <span style={{ color: '#556', fontSize: 10 }}>
              {vol.type}
            </span>
          </div>
        ))}
      </div>

      {/* Surfaces group */}
      <div>
        <Dropdown
          menu={{ items: surfacesContextMenuItems }}
          trigger={['contextMenu']}
        >
          <div
            style={groupHeaderStyle}
            onClick={() => setExpandedSurfaces(!expandedSurfaces)}
          >
            <FolderOutlined style={{ fontSize: 11 }} />
            <BorderInnerOutlined style={{ fontSize: 11, color: '#44cc44' }} />
            <span>Surfaces</span>
            <span style={{ color: '#556', fontSize: 10, marginLeft: 'auto' }}>
              {meshSurfaces.length}
            </span>
          </div>
        </Dropdown>
        {expandedSurfaces && (
          <>
            {assignedSurfaces.map((surf) => (
              <div
                key={surf.id}
                style={itemStyle(selectedMeshSurfaceId === surf.id)}
                onClick={() => handleSurfaceClick(surf.id)}
                onMouseEnter={(e) => {
                  if (selectedMeshSurfaceId !== surf.id) {
                    e.currentTarget.style.background = '#161630';
                  }
                }}
                onMouseLeave={(e) => {
                  if (selectedMeshSurfaceId !== surf.id) {
                    e.currentTarget.style.background = 'transparent';
                  }
                }}
              >
                <ColorDot color={surf.color} size={10} />
                <span style={{ flex: 1 }}>{surf.name}</span>
                <span style={{ color: '#556', fontSize: 10 }}>
                  {boundaryTypeLabels[surf.boundaryType]}
                </span>
              </div>
            ))}
            {unassignedCount > 0 && (
              <div
                style={{
                  ...itemStyle(false),
                  color: '#555',
                  fontStyle: 'italic',
                }}
              >
                <ColorDot color="#333" size={8} />
                <span>(unassigned faces: {unassignedCount})</span>
              </div>
            )}
            {/* Right-click hint */}
            <div
              style={{
                padding: '4px 8px 4px 24px',
                fontSize: 10,
                color: '#444',
                fontStyle: 'italic',
              }}
            >
              Right-click "Surfaces" to add boundary
            </div>
          </>
        )}
      </div>
    </div>
  );
};

export default MeshZoneTree;

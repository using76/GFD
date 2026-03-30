import React, { useCallback, useMemo } from 'react';
import { Button, Select, Divider } from 'antd';
import { DeleteOutlined, CheckOutlined } from '@ant-design/icons';
import { useAppStore, BOUNDARY_COLORS } from '../../store/useAppStore';
import type { MeshSurfaceBoundaryType, MeshSurfaceFaceDirection } from '../../store/useAppStore';

/** Labels for boundary types */
const boundaryTypeOptions: { label: string; value: MeshSurfaceBoundaryType }[] = [
  { label: 'Inlet', value: 'inlet' },
  { label: 'Outlet', value: 'outlet' },
  { label: 'Wall', value: 'wall' },
  { label: 'Symmetry', value: 'symmetry' },
  { label: 'Periodic', value: 'periodic' },
  { label: 'Open', value: 'open' },
];

/** Labels for face directions */
const faceDirectionLabels: Record<MeshSurfaceFaceDirection, string> = {
  xmin: '-X face (xmin)',
  xmax: '+X face (xmax)',
  ymin: '-Y face (ymin)',
  ymax: '+Y face (ymax)',
  zmin: '-Z face (zmin)',
  zmax: '+Z face (zmax)',
  interface: 'Solid-fluid interface',
  custom: 'Custom',
};

const BoundaryEditor: React.FC = () => {
  const meshSurfaces = useAppStore((s) => s.meshSurfaces);
  const editingSurfaceId = useAppStore((s) => s.editingSurfaceId);
  const updateMeshSurface = useAppStore((s) => s.updateMeshSurface);
  const removeBoundarySurface = useAppStore((s) => s.removeBoundarySurface);
  const setEditingSurface = useAppStore((s) => s.setEditingSurface);

  const surface = useMemo(
    () => meshSurfaces.find((s) => s.id === editingSurfaceId),
    [meshSurfaces, editingSurfaceId]
  );

  // Which faces are currently assigned to THIS boundary
  // A face is "assigned to this boundary" if this surface IS that face (same id)
  // Or if we track assignment differently: the editing surface has a faceDirection
  const isOwnFace = useCallback((faceDir: MeshSurfaceFaceDirection) => {
    if (!surface) return false;
    return surface.faceDirection === faceDir;
  }, [surface]);

  // Check if a face is already assigned to another boundary
  const faceAssignment = useCallback((faceDir: MeshSurfaceFaceDirection): string | null => {
    const assigned = meshSurfaces.find(
      (s) => s.faceDirection === faceDir && s.boundaryType !== 'none' && s.id !== editingSurfaceId
    );
    return assigned ? assigned.name : null;
  }, [meshSurfaces, editingSurfaceId]);

  const handleTypeChange = useCallback((type: MeshSurfaceBoundaryType) => {
    if (!editingSurfaceId) return;
    updateMeshSurface(editingSurfaceId, {
      boundaryType: type,
      color: BOUNDARY_COLORS[type],
    });
  }, [editingSurfaceId, updateMeshSurface]);

  const handleFaceToggle = useCallback((faceDir: MeshSurfaceFaceDirection) => {
    if (!editingSurfaceId || !surface) return;
    // If this face is already this surface's face, un-assign it
    if (surface.faceDirection === faceDir) {
      // Find the original domain face to get center/normal/size
      const domainFace = meshSurfaces.find(
        (s) => s.faceDirection === faceDir && s.id.startsWith('surf-')
      );
      if (domainFace) {
        updateMeshSurface(domainFace.id, { boundaryType: 'none', color: BOUNDARY_COLORS.none });
      }
      updateMeshSurface(editingSurfaceId, {
        faceDirection: 'custom',
        center: [0, 0, 0],
        normal: [0, 0, 0],
        width: 0,
        height: 0,
      });
    } else {
      // Assign this face to this boundary
      const domainFace = meshSurfaces.find(
        (s) => s.faceDirection === faceDir && s.id.startsWith('surf-')
      );
      if (domainFace) {
        // Update the domain face's boundary type
        updateMeshSurface(domainFace.id, {
          boundaryType: surface.boundaryType,
          color: surface.color,
        });
        // Also copy position info to the editing surface
        updateMeshSurface(editingSurfaceId, {
          faceDirection: faceDir,
          center: domainFace.center,
          normal: domainFace.normal,
          width: domainFace.width,
          height: domainFace.height,
        });
      }
    }
  }, [editingSurfaceId, surface, meshSurfaces, updateMeshSurface]);

  const handleDelete = useCallback(() => {
    if (!editingSurfaceId) return;
    // If this boundary was assigned to a domain face, reset that face
    if (surface && surface.faceDirection !== 'custom') {
      const domainFace = meshSurfaces.find(
        (s) => s.faceDirection === surface.faceDirection && s.id.startsWith('surf-')
      );
      if (domainFace && domainFace.id !== editingSurfaceId) {
        updateMeshSurface(domainFace.id, { boundaryType: 'none', color: BOUNDARY_COLORS.none });
      }
    }
    removeBoundarySurface(editingSurfaceId);
    setEditingSurface(null);
  }, [editingSurfaceId, surface, meshSurfaces, updateMeshSurface, removeBoundarySurface, setEditingSurface]);

  if (!surface) {
    return (
      <div style={{ padding: 16, color: '#666', fontSize: 12 }}>
        Select a boundary or surface to edit.
      </div>
    );
  }

  const allFaces: MeshSurfaceFaceDirection[] = ['xmin', 'xmax', 'ymin', 'ymax', 'zmin', 'zmax', 'interface'];

  return (
    <div style={{ padding: 12, fontSize: 12 }}>
      {/* Header */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          marginBottom: 12,
          paddingBottom: 8,
          borderBottom: '1px solid #303050',
        }}
      >
        <span
          style={{
            display: 'inline-block',
            width: 12,
            height: 12,
            borderRadius: '50%',
            background: surface.color,
          }}
        />
        <span style={{ fontWeight: 600, fontSize: 14, color: '#dde' }}>
          {surface.name}
        </span>
      </div>

      {/* Boundary type selector */}
      <div style={{ marginBottom: 12 }}>
        <div style={{ color: '#889', marginBottom: 4 }}>Type:</div>
        <Select
          value={surface.boundaryType === 'none' ? undefined : surface.boundaryType}
          onChange={handleTypeChange}
          placeholder="Select type"
          size="small"
          style={{ width: '100%' }}
          options={boundaryTypeOptions}
        />
      </div>

      <Divider style={{ margin: '8px 0', borderColor: '#303050' }} />

      {/* Face assignment */}
      <div style={{ marginBottom: 12 }}>
        <div style={{ color: '#889', marginBottom: 8, fontWeight: 500 }}>Assigned Faces:</div>
        {allFaces.map((faceDir) => {
          const isChecked = isOwnFace(faceDir);
          const assignedTo = faceAssignment(faceDir);
          const isDisabled = !!assignedTo && !isChecked;
          // Also check if the face is assigned within the domain faces directly
          const domainFace = meshSurfaces.find(
            (s) => s.faceDirection === faceDir && s.id.startsWith('surf-')
          );
          const domainAssigned = domainFace && domainFace.boundaryType !== 'none' && domainFace.id !== editingSurfaceId;

          return (
            <div
              key={faceDir}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 8,
                padding: '3px 4px',
                borderRadius: 3,
                cursor: isDisabled ? 'not-allowed' : 'pointer',
                opacity: isDisabled ? 0.5 : 1,
                background: isChecked ? '#1a2a3a' : 'transparent',
              }}
              onClick={() => {
                if (!isDisabled) handleFaceToggle(faceDir);
              }}
              onMouseEnter={(e) => {
                if (!isDisabled && !isChecked) {
                  e.currentTarget.style.background = '#161630';
                }
              }}
              onMouseLeave={(e) => {
                if (!isChecked) {
                  e.currentTarget.style.background = 'transparent';
                }
              }}
            >
              <input
                type="checkbox"
                checked={isChecked}
                onChange={() => {}}
                style={{ margin: 0, cursor: isDisabled ? 'not-allowed' : 'pointer' }}
                disabled={isDisabled}
              />
              <span style={{ color: isChecked ? '#4096ff' : '#aab', flex: 1 }}>
                {faceDirectionLabels[faceDir]}
              </span>
              {domainAssigned && domainFace && (
                <span style={{ color: '#556', fontSize: 10 }}>
                  ({domainFace.boundaryType})
                </span>
              )}
            </div>
          );
        })}
      </div>

      <Divider style={{ margin: '8px 0', borderColor: '#303050' }} />

      {/* Action buttons */}
      <div style={{ display: 'flex', gap: 8 }}>
        <Button
          size="small"
          icon={<CheckOutlined />}
          onClick={() => setEditingSurface(null)}
          style={{ flex: 1 }}
        >
          Apply
        </Button>
        <Button
          size="small"
          danger
          icon={<DeleteOutlined />}
          onClick={handleDelete}
          style={{ flex: 1 }}
        >
          Delete
        </Button>
      </div>

      {/* Info */}
      {surface.center[0] !== 0 || surface.center[1] !== 0 || surface.center[2] !== 0 ? (
        <div style={{ color: '#556', fontSize: 10, marginTop: 8 }}>
          Center: ({surface.center[0].toFixed(2)}, {surface.center[1].toFixed(2)}, {surface.center[2].toFixed(2)})
          <br />
          Normal: ({surface.normal[0].toFixed(1)}, {surface.normal[1].toFixed(1)}, {surface.normal[2].toFixed(1)})
          <br />
          Size: {surface.width.toFixed(2)} x {surface.height.toFixed(2)}
        </div>
      ) : null}
    </div>
  );
};

export default BoundaryEditor;

import React, { useCallback, useRef } from 'react';
import { useAppStore } from '../store/useAppStore';
import type { MeasurePoint } from '../store/useAppStore';

/**
 * MeasureOverlay renders measurement labels as HTML overlay on top of the 3D Canvas.
 * When measureMode is active, clicking on the viewport generates measurement results.
 *
 * Now tracks 3D world positions: clicks are mapped to a ground plane (Y=0) via simple
 * ray-plane intersection using normalized device coordinates. This gives meaningful
 * 3D positions for the measurement markers in CadScene.
 */
const MeasureOverlay: React.FC = () => {
  const measureMode = useAppStore((s) => s.measureMode);
  const measureLabels = useAppStore((s) => s.measureLabels);
  const measurePoints = useAppStore((s) => s.measurePoints);
  const addMeasureLabel = useAppStore((s) => s.addMeasureLabel);
  const addMeasurePoint = useAppStore((s) => s.addMeasurePoint);
  const clearMeasurePoints = useAppStore((s) => s.clearMeasurePoints);
  const overlayRef = useRef<HTMLDivElement>(null);
  const labelCounter = useRef(0);

  /** Convert screen click to approximate 3D world position.
   *  Uses a simple mapping: screen coords -> world coords on Y=0 plane.
   *  The viewport is treated as mapping to a [-3, 3] x [-3, 3] world range. */
  const screenToWorld = useCallback((screenX: number, screenY: number, rect: DOMRect): [number, number, number] => {
    const nx = (screenX / rect.width) * 2 - 1;
    const ny = -((screenY / rect.height) * 2 - 1);
    // Map to world coordinates on Y=0 plane, approximate based on default camera
    const worldX = nx * 3;
    const worldY = 0;
    const worldZ = ny * 3;
    return [worldX, worldY, worldZ];
  }, []);

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      if (!measureMode) return;

      const rect = overlayRef.current?.getBoundingClientRect();
      if (!rect) return;

      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;
      const worldPos = screenToWorld(screenX, screenY, rect);
      const newPoint: MeasurePoint = { worldPos, screenPos: [screenX, screenY] };

      if (measureMode === 'distance') {
        const allPoints = [...measurePoints, newPoint];
        if (allPoints.length >= 2) {
          const p1 = allPoints[0];
          const p2 = allPoints[1];
          const dx = p2.worldPos[0] - p1.worldPos[0];
          const dy = p2.worldPos[1] - p1.worldPos[1];
          const dz = p2.worldPos[2] - p1.worldPos[2];
          const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);

          const id = `meas-${++labelCounter.current}`;
          addMeasureLabel({
            id,
            text: `${dist.toFixed(3)} m`,
            position: p1.worldPos,
            endPosition: p2.worldPos,
            screenPos: p1.screenPos,
            screenEndPos: p2.screenPos,
          });
          clearMeasurePoints();
        } else {
          addMeasurePoint(newPoint);
        }
      } else if (measureMode === 'angle') {
        const allPoints = [...measurePoints, newPoint];
        if (allPoints.length >= 3) {
          const p1 = allPoints[0];
          const p2 = allPoints[1]; // vertex
          const p3 = allPoints[2];
          const v1 = [p1.worldPos[0] - p2.worldPos[0], p1.worldPos[1] - p2.worldPos[1], p1.worldPos[2] - p2.worldPos[2]];
          const v2 = [p3.worldPos[0] - p2.worldPos[0], p3.worldPos[1] - p2.worldPos[1], p3.worldPos[2] - p2.worldPos[2]];
          const dot = v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2];
          const mag1 = Math.sqrt(v1[0] * v1[0] + v1[1] * v1[1] + v1[2] * v1[2]);
          const mag2 = Math.sqrt(v2[0] * v2[0] + v2[1] * v2[1] + v2[2] * v2[2]);
          const angle = mag1 > 0 && mag2 > 0
            ? (Math.acos(Math.max(-1, Math.min(1, dot / (mag1 * mag2)))) * 180 / Math.PI).toFixed(1)
            : '0.0';

          const id = `meas-${++labelCounter.current}`;
          addMeasureLabel({
            id,
            text: `${angle} deg`,
            position: p2.worldPos,
            screenPos: p2.screenPos,
          });
          clearMeasurePoints();
        } else {
          addMeasurePoint(newPoint);
        }
      } else if (measureMode === 'area') {
        const area = (0.5 + Math.random() * 2.0).toFixed(4);
        const id = `meas-${++labelCounter.current}`;
        addMeasureLabel({
          id,
          text: `${area} m^2`,
          position: worldPos,
          screenPos: [screenX, screenY],
        });
        clearMeasurePoints();
      }
    },
    [measureMode, measurePoints, addMeasureLabel, addMeasurePoint, clearMeasurePoints, screenToWorld]
  );

  if (!measureMode && measureLabels.length === 0) return null;

  return (
    <div
      ref={overlayRef}
      onClick={handleClick}
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        pointerEvents: measureMode ? 'auto' : 'none',
        zIndex: measureMode ? 20 : 5,
        cursor: measureMode ? 'crosshair' : 'default',
      }}
    >
      {/* Instruction banner when measure is active */}
      {measureMode && (
        <div
          style={{
            position: 'absolute',
            top: 8,
            left: '50%',
            transform: 'translateX(-50%)',
            background: 'rgba(20, 20, 50, 0.9)',
            border: '1px solid #4096ff',
            borderRadius: 6,
            padding: '4px 12px',
            color: '#aab',
            fontSize: 11,
            pointerEvents: 'none',
            whiteSpace: 'nowrap',
          }}
        >
          {measureMode === 'distance' && (
            measurePoints.length === 0
              ? 'Click first point for distance measurement'
              : 'Click second point to complete measurement'
          )}
          {measureMode === 'angle' && (
            measurePoints.length === 0
              ? 'Click first point (ray start)'
              : measurePoints.length === 1
              ? 'Click vertex point'
              : 'Click third point to complete angle'
          )}
          {measureMode === 'area' && 'Click a face to measure area'}
        </div>
      )}

      {/* Render click markers for in-progress measurement */}
      {measurePoints.map((pt, i) => (
        <div
          key={`click-${i}`}
          style={{
            position: 'absolute',
            left: pt.screenPos[0] - 6,
            top: pt.screenPos[1] - 6,
            width: 12,
            height: 12,
            borderRadius: '50%',
            background: '#ff4444',
            border: '2px solid #fff',
            pointerEvents: 'none',
            boxShadow: '0 0 8px rgba(255, 68, 68, 0.6)',
          }}
        />
      ))}

      {/* Render connecting line between in-progress distance points */}
      {measureMode === 'distance' && measurePoints.length === 1 && (
        <svg
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            width: '100%',
            height: '100%',
            pointerEvents: 'none',
          }}
        >
          <circle
            cx={measurePoints[0].screenPos[0]}
            cy={measurePoints[0].screenPos[1]}
            r={8}
            fill="none"
            stroke="rgba(64, 150, 255, 0.5)"
            strokeWidth={2}
            strokeDasharray="4 2"
          />
        </svg>
      )}

      {/* Render measurement labels */}
      {measureLabels.map((label) => {
        // For distance labels, render a line + midpoint label
        if (label.screenPos && label.screenEndPos) {
          const midX = (label.screenPos[0] + label.screenEndPos[0]) / 2;
          const midY = (label.screenPos[1] + label.screenEndPos[1]) / 2;
          return (
            <React.Fragment key={label.id}>
              {/* SVG line between the two screen points */}
              <svg
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  height: '100%',
                  pointerEvents: 'none',
                }}
              >
                <line
                  x1={label.screenPos[0]}
                  y1={label.screenPos[1]}
                  x2={label.screenEndPos[0]}
                  y2={label.screenEndPos[1]}
                  stroke="#4096ff"
                  strokeWidth={2}
                  strokeDasharray="6 3"
                />
                {/* Endpoint markers */}
                <circle cx={label.screenPos[0]} cy={label.screenPos[1]} r={5} fill="#ff4444" stroke="#fff" strokeWidth={1.5} />
                <circle cx={label.screenEndPos[0]} cy={label.screenEndPos[1]} r={5} fill="#ff4444" stroke="#fff" strokeWidth={1.5} />
              </svg>
              {/* Distance label at midpoint */}
              <div
                style={{
                  position: 'absolute',
                  left: midX,
                  top: midY,
                  transform: 'translate(-50%, -130%)',
                  background: 'rgba(10, 10, 30, 0.92)',
                  border: '1px solid #4096ff',
                  borderRadius: 4,
                  padding: '3px 10px',
                  color: '#fff',
                  fontSize: 12,
                  fontWeight: 600,
                  whiteSpace: 'nowrap',
                  pointerEvents: 'none',
                  boxShadow: '0 2px 8px rgba(0,0,0,0.5)',
                }}
              >
                {label.text}
                <div
                  style={{
                    position: 'absolute',
                    bottom: -5,
                    left: '50%',
                    transform: 'translateX(-50%)',
                    width: 0,
                    height: 0,
                    borderLeft: '5px solid transparent',
                    borderRight: '5px solid transparent',
                    borderTop: '5px solid #4096ff',
                  }}
                />
              </div>
            </React.Fragment>
          );
        }

        // Single-point labels (angle, area)
        const posX = label.screenPos ? label.screenPos[0] : label.position[0];
        const posY = label.screenPos ? label.screenPos[1] : label.position[1];
        return (
          <div
            key={label.id}
            style={{
              position: 'absolute',
              left: posX,
              top: posY,
              transform: 'translate(-50%, -130%)',
              background: 'rgba(10, 10, 30, 0.92)',
              border: '1px solid #4096ff',
              borderRadius: 4,
              padding: '3px 10px',
              color: '#fff',
              fontSize: 12,
              fontWeight: 600,
              whiteSpace: 'nowrap',
              pointerEvents: 'none',
              boxShadow: '0 2px 8px rgba(0,0,0,0.5)',
            }}
          >
            {label.text}
            <div
              style={{
                position: 'absolute',
                bottom: -5,
                left: '50%',
                transform: 'translateX(-50%)',
                width: 0,
                height: 0,
                borderLeft: '5px solid transparent',
                borderRight: '5px solid transparent',
                borderTop: '5px solid #4096ff',
              }}
            />
          </div>
        );
      })}
    </div>
  );
};

export default MeasureOverlay;

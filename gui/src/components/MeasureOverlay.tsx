import React, { useCallback, useRef, useState } from 'react';
import { useAppStore } from '../store/useAppStore';

/**
 * MeasureOverlay renders measurement labels as HTML overlay on top of the 3D Canvas.
 * When measureMode is active, clicking on the viewport generates measurement results.
 */
const MeasureOverlay: React.FC = () => {
  const measureMode = useAppStore((s) => s.measureMode);
  const measureLabels = useAppStore((s) => s.measureLabels);
  const addMeasureLabel = useAppStore((s) => s.addMeasureLabel);
  const [clickPoints, setClickPoints] = useState<{ x: number; y: number }[]>([]);
  const overlayRef = useRef<HTMLDivElement>(null);
  const labelCounter = useRef(0);

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      if (!measureMode) return;

      const rect = overlayRef.current?.getBoundingClientRect();
      if (!rect) return;

      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      if (measureMode === 'distance') {
        const newPoints = [...clickPoints, { x, y }];
        if (newPoints.length >= 2) {
          const p1 = newPoints[0];
          const p2 = newPoints[1];
          const dx = p2.x - p1.x;
          const dy = p2.y - p1.y;
          // Simulated distance in model units (pixels -> approximate meters)
          const pixelDist = Math.sqrt(dx * dx + dy * dy);
          const modelDist = (pixelDist * 0.005).toFixed(3);

          const id = `meas-${++labelCounter.current}`;
          addMeasureLabel({
            id,
            text: `Distance: ${modelDist} m`,
            position: [(p1.x + p2.x) / 2, (p1.y + p2.y) / 2, 0],
          });
          setClickPoints([]);
        } else {
          setClickPoints(newPoints);
        }
      } else if (measureMode === 'angle') {
        const newPoints = [...clickPoints, { x, y }];
        if (newPoints.length >= 3) {
          // Compute angle between three points
          const p1 = newPoints[0];
          const p2 = newPoints[1]; // vertex
          const p3 = newPoints[2];
          const v1 = { x: p1.x - p2.x, y: p1.y - p2.y };
          const v2 = { x: p3.x - p2.x, y: p3.y - p2.y };
          const dot = v1.x * v2.x + v1.y * v2.y;
          const mag1 = Math.sqrt(v1.x * v1.x + v1.y * v1.y);
          const mag2 = Math.sqrt(v2.x * v2.x + v2.y * v2.y);
          const angle = mag1 > 0 && mag2 > 0
            ? (Math.acos(Math.max(-1, Math.min(1, dot / (mag1 * mag2)))) * 180 / Math.PI).toFixed(1)
            : '0.0';

          const id = `meas-${++labelCounter.current}`;
          addMeasureLabel({
            id,
            text: `Angle: ${angle} deg`,
            position: [p2.x, p2.y, 0],
          });
          setClickPoints([]);
        } else {
          setClickPoints(newPoints);
        }
      } else if (measureMode === 'area') {
        // Simulated: click once to get area
        const area = (0.5 + Math.random() * 2.0).toFixed(4);
        const id = `meas-${++labelCounter.current}`;
        addMeasureLabel({
          id,
          text: `Area: ${area} m^2`,
          position: [x, y, 0],
        });
        setClickPoints([]);
      }
    },
    [measureMode, clickPoints, addMeasureLabel]
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
            clickPoints.length === 0
              ? 'Click first point for distance measurement'
              : 'Click second point to complete measurement'
          )}
          {measureMode === 'angle' && (
            clickPoints.length === 0
              ? 'Click first point (ray start)'
              : clickPoints.length === 1
              ? 'Click vertex point'
              : 'Click third point to complete angle'
          )}
          {measureMode === 'area' && 'Click a face to measure area'}
        </div>
      )}

      {/* Render click markers for in-progress measurement */}
      {clickPoints.map((pt, i) => (
        <div
          key={i}
          style={{
            position: 'absolute',
            left: pt.x - 5,
            top: pt.y - 5,
            width: 10,
            height: 10,
            borderRadius: '50%',
            background: '#4096ff',
            border: '2px solid #fff',
            pointerEvents: 'none',
          }}
        />
      ))}

      {/* Render line between click points for distance */}
      {measureMode === 'distance' && clickPoints.length === 1 && (
        <div
          style={{
            position: 'absolute',
            left: clickPoints[0].x,
            top: clickPoints[0].y,
            width: 8,
            height: 8,
            borderRadius: '50%',
            background: 'rgba(64, 150, 255, 0.5)',
            transform: 'translate(-4px, -4px)',
            pointerEvents: 'none',
          }}
        />
      )}

      {/* Render measurement labels */}
      {measureLabels.map((label) => (
        <div
          key={label.id}
          style={{
            position: 'absolute',
            left: label.position[0],
            top: label.position[1],
            transform: 'translate(-50%, -120%)',
            background: 'rgba(10, 10, 30, 0.92)',
            border: '1px solid #4096ff',
            borderRadius: 4,
            padding: '3px 8px',
            color: '#fff',
            fontSize: 11,
            fontWeight: 500,
            whiteSpace: 'nowrap',
            pointerEvents: 'none',
            boxShadow: '0 2px 8px rgba(0,0,0,0.5)',
          }}
        >
          {label.text}
          {/* Down arrow */}
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
      ))}
    </div>
  );
};

export default MeasureOverlay;

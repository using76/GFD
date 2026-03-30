import React from 'react';
import { useAppStore } from '../store/useAppStore';

/**
 * MeasureOverlay renders measurement labels as HTML overlay on top of the 3D Canvas.
 * All measurement click handling is done by CadScene's MeasureClickHandler via Three.js raycasting.
 * This component is purely visual (pointerEvents: none).
 */
const MeasureOverlay: React.FC = () => {
  const measureMode = useAppStore((s) => s.measureMode);
  const measureLabels = useAppStore((s) => s.measureLabels);
  const measurePoints = useAppStore((s) => s.measurePoints);

  if (!measureMode && measureLabels.length === 0) return null;

  return (
    <div
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        pointerEvents: 'none',
        zIndex: measureMode ? 20 : 5,
        cursor: 'default',
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

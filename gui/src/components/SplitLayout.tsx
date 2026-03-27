import React, { useState, useCallback, useRef } from 'react';

interface SplitLayoutProps {
  left: React.ReactNode;
  center: React.ReactNode;
  right: React.ReactNode;
  defaultLeftWidth?: number;
  defaultRightWidth?: number;
  minPanelWidth?: number;
}

const HANDLE_WIDTH = 4;

const SplitLayout: React.FC<SplitLayoutProps> = ({
  left,
  center,
  right,
  defaultLeftWidth = 240,
  defaultRightWidth = 280,
  minPanelWidth = 160,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [leftWidth, setLeftWidth] = useState(defaultLeftWidth);
  const [rightWidth, setRightWidth] = useState(defaultRightWidth);
  const dragging = useRef<'left' | 'right' | null>(null);

  const onMouseDown = useCallback(
    (handle: 'left' | 'right') => (e: React.MouseEvent) => {
      e.preventDefault();
      dragging.current = handle;

      const startX = e.clientX;
      const startLeft = leftWidth;
      const startRight = rightWidth;

      const onMouseMove = (ev: MouseEvent) => {
        if (!containerRef.current) return;
        const dx = ev.clientX - startX;
        const totalWidth = containerRef.current.clientWidth;

        if (dragging.current === 'left') {
          const newLeft = Math.max(
            minPanelWidth,
            Math.min(startLeft + dx, totalWidth - rightWidth - minPanelWidth - 2 * HANDLE_WIDTH)
          );
          setLeftWidth(newLeft);
        } else {
          const newRight = Math.max(
            minPanelWidth,
            Math.min(startRight - dx, totalWidth - leftWidth - minPanelWidth - 2 * HANDLE_WIDTH)
          );
          setRightWidth(newRight);
        }
      };

      const onMouseUp = () => {
        dragging.current = null;
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', onMouseUp);
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
      };

      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    },
    [leftWidth, rightWidth, minPanelWidth]
  );

  return (
    <div
      ref={containerRef}
      style={{
        display: 'flex',
        width: '100%',
        height: '100%',
        overflow: 'hidden',
      }}
    >
      {/* Left panel */}
      <div
        style={{
          width: leftWidth,
          minWidth: minPanelWidth,
          height: '100%',
          overflow: 'auto',
          background: '#141414',
          borderRight: '1px solid #303030',
        }}
      >
        {left}
      </div>

      {/* Left drag handle */}
      <div
        onMouseDown={onMouseDown('left')}
        style={{
          width: HANDLE_WIDTH,
          cursor: 'col-resize',
          background: '#222',
          flexShrink: 0,
        }}
      />

      {/* Center panel */}
      <div
        style={{
          flex: 1,
          height: '100%',
          overflow: 'hidden',
          background: '#1a1a2e',
          minWidth: 200,
        }}
      >
        {center}
      </div>

      {/* Right drag handle */}
      <div
        onMouseDown={onMouseDown('right')}
        style={{
          width: HANDLE_WIDTH,
          cursor: 'col-resize',
          background: '#222',
          flexShrink: 0,
        }}
      />

      {/* Right panel */}
      <div
        style={{
          width: rightWidth,
          minWidth: minPanelWidth,
          height: '100%',
          overflow: 'auto',
          background: '#141414',
          borderLeft: '1px solid #303030',
        }}
      >
        {right}
      </div>
    </div>
  );
};

export default SplitLayout;

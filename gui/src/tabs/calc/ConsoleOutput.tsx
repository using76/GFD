import React, { useEffect, useRef } from 'react';
import { useAppStore } from '../../store/useAppStore';

const ConsoleOutput: React.FC = () => {
  const consoleLines = useAppStore((s) => s.consoleLines);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [consoleLines]);

  return (
    <div
      ref={containerRef}
      style={{
        width: '100%',
        height: '100%',
        overflow: 'auto',
        background: '#0d0d0d',
        fontFamily: "'Cascadia Code', 'Fira Code', 'Consolas', monospace",
        fontSize: 12,
        lineHeight: 1.6,
        padding: 8,
        color: '#c0c0c0',
      }}
    >
      {consoleLines.length === 0 ? (
        <span style={{ color: '#555' }}>Solver output will appear here...</span>
      ) : (
        consoleLines.map((line, i) => (
          <div
            key={i}
            style={{
              color: line.startsWith('[GFD]')
                ? '#1668dc'
                : line.includes('error') || line.includes('Error')
                ? '#ff4d4f'
                : '#c0c0c0',
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-all',
            }}
          >
            {line}
          </div>
        ))
      )}
    </div>
  );
};

export default ConsoleOutput;

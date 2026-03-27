import React, { useEffect, useRef } from 'react';
import { useAppStore } from '../../store/useAppStore';

function getLineColor(line: string): string {
  if (line.includes('[GFD]') && (line.includes('===') || line.includes('---'))) return '#445';
  if (line.includes('[GFD]') && line.includes('CONVERGED')) return '#52c41a';
  if (line.includes('[GFD]')) return '#1668dc';
  if (line.includes('WARNING') || line.includes('warning')) return '#faad14';
  if (line.includes('ERROR') || line.includes('error') || line.includes('Error')) return '#ff4d4f';
  if (line.includes('[Iter')) return '#a0a0b0';
  return '#c0c0c0';
}

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
        background: '#0a0a10',
        fontFamily: "'Cascadia Code', 'Fira Code', 'Consolas', monospace",
        fontSize: 11,
        lineHeight: 1.5,
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
              color: getLineColor(line),
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-all',
              fontWeight: line.includes('CONVERGED') ? 700 : 400,
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

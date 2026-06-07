/**
 * Data-driven ribbon (Phase 4). Tabs and buttons are derived entirely from the
 * command registry via buildRibbonModel — registering a command makes it appear
 * here automatically. Clicking a command opens its schema-driven form panel.
 */

import { useMemo, useState } from 'react';
import { buildRibbonModel } from '../core';
import { useCore } from './CoreContext';

export function RibbonFromRegistry({ onPick }: { onPick: (commandId: string) => void }) {
  const core = useCore();
  const tabs = useMemo(() => buildRibbonModel(core.registry), [core]);
  const [activeTab, setActiveTab] = useState(tabs[0]?.category ?? 'geometry');

  const current = tabs.find((t) => t.category === activeTab) ?? tabs[0];

  return (
    <div style={{ borderBottom: '1px solid #333', background: '#1e1e1e', color: '#ddd' }}>
      <div style={{ display: 'flex', gap: 2, padding: '4px 6px 0' }}>
        {tabs.map((t) => (
          <button
            key={t.category}
            onClick={() => setActiveTab(t.category)}
            style={{
              padding: '4px 12px',
              border: 'none',
              borderBottom: t.category === activeTab ? '2px solid #4096ff' : '2px solid transparent',
              background: 'transparent',
              color: t.category === activeTab ? '#fff' : '#aaa',
              cursor: 'pointer',
            }}
          >
            {t.label}
          </button>
        ))}
      </div>
      <div style={{ display: 'flex', gap: 12, padding: '8px 10px', flexWrap: 'wrap' }}>
        {current?.groups.map((g) => (
          <div key={g.name} style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
              {g.commands.map((c) => (
                <button
                  key={c.id}
                  title={c.description}
                  onClick={() => onPick(c.id)}
                  style={{
                    padding: '4px 8px',
                    border: '1px solid #3a3a3a',
                    borderRadius: 4,
                    background: '#2a2a2a',
                    color: '#ddd',
                    cursor: 'pointer',
                    fontSize: 12,
                  }}
                >
                  {c.titleKo ?? c.title}
                </button>
              ))}
            </div>
            <div style={{ fontSize: 10, color: '#666', textAlign: 'center' }}>{g.name}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

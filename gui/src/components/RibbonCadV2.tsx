import React from 'react';
import {
  ExperimentOutlined,
  EyeOutlined,
  ColumnWidthOutlined,
  ToolOutlined,
} from '@ant-design/icons';

/**
 * Placeholder Design/Display/Measure/Repair ribbons.
 *
 * The old mesh-based implementations were removed on 2026-04-20 so the tabs
 * can be rebuilt on top of the pure-Rust CAD kernel (crates/gfd-cad-*).
 * See docs/CAD_KERNEL_PLAN.md for the phased plan. Each sub-ribbon shows
 * the phase it is waiting on and a short status line.
 */

type Props = { tab: 'design' | 'display' | 'measure' | 'repair' };

const COPY: Record<Props['tab'], { title: string; phase: string; icon: React.ReactNode }> = {
  design: {
    title: 'Design — CAD kernel rebuild',
    phase: 'Phase 5/8: Part Design features + FreeCAD-style Design tab',
    icon: <ExperimentOutlined />,
  },
  display: {
    title: 'Display — B-Rep visualization',
    phase: 'Phase 9: rendering modes + section/exploded views',
    icon: <EyeOutlined />,
  },
  measure: {
    title: 'Measure — exact B-Rep queries',
    phase: 'Phase 10: analytical area / volume / inertia',
    icon: <ColumnWidthOutlined />,
  },
  repair: {
    title: 'Repair — Shape healing',
    phase: 'Phase 11: sew / fix wires / remove small features',
    icon: <ToolOutlined />,
  },
};

const RibbonCadV2: React.FC<Props> = ({ tab }) => {
  const { title, phase, icon } = COPY[tab];
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 12,
        padding: '0 12px',
        color: '#bbb',
        fontSize: 12,
        lineHeight: 1.3,
      }}
    >
      <span style={{ fontSize: 22, color: '#4096ff' }}>{icon}</span>
      <div style={{ display: 'flex', flexDirection: 'column' }}>
        <span style={{ color: '#eee', fontWeight: 600 }}>{title}</span>
        <span style={{ color: '#889', fontSize: 10 }}>{phase}</span>
      </div>
      <div style={{ flex: 1 }} />
      <span style={{ fontSize: 10, color: '#667' }}>
        See docs/CAD_KERNEL_PLAN.md
      </span>
    </div>
  );
};

export const DesignRibbonV2: React.FC = () => <RibbonCadV2 tab="design" />;
export const DisplayRibbonV2: React.FC = () => <RibbonCadV2 tab="display" />;
export const MeasureRibbonV2: React.FC = () => <RibbonCadV2 tab="measure" />;
export const RepairRibbonV2: React.FC = () => <RibbonCadV2 tab="repair" />;

export default RibbonCadV2;

import React, { useState, useCallback, useRef, useEffect } from 'react';
import { Button, InputNumber, Form, Badge, Tag, Space, Divider, message, Tooltip } from 'antd';
import {
  SearchOutlined,
  ThunderboltOutlined,
  CheckCircleOutlined,
  WarningOutlined,
  UndoOutlined,
  StepForwardOutlined,
  CloseCircleOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';
import type { DefeatureIssueKind, DefeatureIssue } from '../../store/useAppStore';

const issueColors: Record<DefeatureIssueKind, string> = {
  small_face: '#ff4d4f',
  short_edge: '#fa8c16',
  small_hole: '#fadb14',
  sliver_face: '#eb2f96',
  gap: '#13c2c2',
};

const issueIcons: Record<DefeatureIssueKind, string> = {
  small_face: '\u25CF',   // filled circle
  short_edge: '\u2501',   // horizontal bar
  small_hole: '\u25CB',   // ring
  sliver_face: '\u25C6',  // diamond
  gap: '\u2504',          // dashed line
};

const issueLabels: Record<DefeatureIssueKind, string> = {
  small_face: 'Small Faces',
  short_edge: 'Short Edges',
  small_hole: 'Small Holes',
  sliver_face: 'Slivers',
  gap: 'Gaps',
};

const issueFixLabels: Record<DefeatureIssueKind, string> = {
  small_face: 'Fix',
  short_edge: 'Fix',
  small_hole: 'Fill',
  sliver_face: 'Fix',
  gap: 'Fix',
};

/** Generate simulated defeaturing issues with 3D positions based on shapes in the store */
function generateIssues(
  shapes: { id: string; position: [number, number, number]; dimensions: Record<string, number> }[],
  thresholds: { minFaceArea: number; minEdgeLength: number; maxHoleDia: number; maxFilletR: number }
): DefeatureIssue[] {
  const issues: DefeatureIssue[] = [];
  let id = 0;

  // If no shapes, generate issues around the origin
  const targets = shapes.length > 0
    ? shapes
    : [{ id: 'origin', position: [0, 0, 0] as [number, number, number], dimensions: { width: 2, height: 2, depth: 2 } }];

  for (const shape of targets) {
    const pos = shape.position;
    const hw = (shape.dimensions.width ?? shape.dimensions.radius ?? 0.5) / 2;
    const hh = (shape.dimensions.height ?? shape.dimensions.radius ?? 0.5) / 2;
    const hd = (shape.dimensions.depth ?? shape.dimensions.radius ?? 0.5) / 2;

    // Small faces near edges/corners of the shape
    const numSmallFaces = 2 + Math.floor(Math.random() * 4);
    for (let i = 0; i < numSmallFaces; i++) {
      const area = Math.random() * thresholds.minFaceArea;
      // Position near a random face of the shape
      const faceIdx = Math.floor(Math.random() * 6);
      const offset: [number, number, number] = [0, 0, 0];
      if (faceIdx === 0) offset[0] = hw;
      else if (faceIdx === 1) offset[0] = -hw;
      else if (faceIdx === 2) offset[1] = hh;
      else if (faceIdx === 3) offset[1] = -hh;
      else if (faceIdx === 4) offset[2] = hd;
      else offset[2] = -hd;
      // Add slight random jitter
      const jitter = () => (Math.random() - 0.5) * 0.2;
      issues.push({
        id: `df-${id++}`,
        kind: 'small_face',
        description: `Face area ${area.toExponential(2)} m\u00B2 (< ${thresholds.minFaceArea})`,
        size: area,
        fixed: false,
        position: [
          pos[0] + offset[0] + jitter(),
          pos[1] + offset[1] + jitter(),
          pos[2] + offset[2] + jitter(),
        ],
        shapeId: shape.id,
      });
    }

    // Short edges along shape edges
    const numShortEdges = 1 + Math.floor(Math.random() * 3);
    for (let i = 0; i < numShortEdges; i++) {
      const len = Math.random() * thresholds.minEdgeLength;
      // Position along an edge
      const edgeAxis = Math.floor(Math.random() * 3);
      const sign1 = Math.random() > 0.5 ? 1 : -1;
      const sign2 = Math.random() > 0.5 ? 1 : -1;
      const extents = [hw, hh, hd];
      const edgePos: [number, number, number] = [pos[0], pos[1], pos[2]];
      const axes = [0, 1, 2].filter((a) => a !== edgeAxis);
      edgePos[axes[0]] += extents[axes[0]] * sign1;
      edgePos[axes[1]] += extents[axes[1]] * sign2;
      edgePos[edgeAxis] += (Math.random() - 0.5) * extents[edgeAxis];
      issues.push({
        id: `df-${id++}`,
        kind: 'short_edge',
        description: `Edge length ${len.toFixed(4)} m (< ${thresholds.minEdgeLength})`,
        size: len,
        fixed: false,
        position: edgePos,
        shapeId: shape.id,
      });
    }

    // Small holes on faces
    const numHoles = 1 + Math.floor(Math.random() * 2);
    for (let i = 0; i < numHoles; i++) {
      const dia = Math.random() * thresholds.maxHoleDia;
      const faceIdx = Math.floor(Math.random() * 6);
      const holePos: [number, number, number] = [pos[0], pos[1], pos[2]];
      if (faceIdx < 2) {
        holePos[0] += faceIdx === 0 ? hw : -hw;
        holePos[1] += (Math.random() - 0.5) * hh;
        holePos[2] += (Math.random() - 0.5) * hd;
      } else if (faceIdx < 4) {
        holePos[1] += faceIdx === 2 ? hh : -hh;
        holePos[0] += (Math.random() - 0.5) * hw;
        holePos[2] += (Math.random() - 0.5) * hd;
      } else {
        holePos[2] += faceIdx === 4 ? hd : -hd;
        holePos[0] += (Math.random() - 0.5) * hw;
        holePos[1] += (Math.random() - 0.5) * hh;
      }
      issues.push({
        id: `df-${id++}`,
        kind: 'small_hole',
        description: `Hole diameter ${dia.toFixed(3)} m (< ${thresholds.maxHoleDia})`,
        size: dia,
        fixed: false,
        position: holePos,
        shapeId: shape.id,
      });
    }

    // Sliver faces (occasional)
    if (Math.random() > 0.4) {
      const ar = 20 + Math.random() * 80;
      const sliverPos: [number, number, number] = [
        pos[0] + (Math.random() - 0.5) * hw * 1.5,
        pos[1] + (Math.random() > 0.5 ? hh : -hh),
        pos[2] + (Math.random() - 0.5) * hd * 1.5,
      ];
      issues.push({
        id: `df-${id++}`,
        kind: 'sliver_face',
        description: `Sliver face with aspect ratio ${ar.toFixed(1)}`,
        size: ar,
        fixed: false,
        position: sliverPos,
        shapeId: shape.id,
      });
    }

    // Gaps between bodies (only if multiple shapes)
    if (targets.length > 1 && Math.random() > 0.5) {
      const gap = Math.random() * 0.05;
      const gapPos: [number, number, number] = [
        pos[0] + hw + gap / 2,
        pos[1],
        pos[2],
      ];
      issues.push({
        id: `df-${id++}`,
        kind: 'gap',
        description: `Gap ${gap.toFixed(4)} m between adjacent bodies`,
        size: gap,
        fixed: false,
        position: gapPos,
        shapeId: shape.id,
      });
    }
  }

  return issues;
}

/** Count issues by kind */
function countByKind(issues: DefeatureIssue[], kind: DefeatureIssueKind): { active: number; total: number } {
  const matching = issues.filter((i) => i.kind === kind);
  return {
    active: matching.filter((i) => !i.fixed).length,
    total: matching.length,
  };
}

const DefeaturingPanel: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const defeatureIssues = useAppStore((s) => s.defeatureIssues);
  const selectedIssueId = useAppStore((s) => s.selectedIssueId);
  const setDefeatureIssues = useAppStore((s) => s.setDefeatureIssues);
  const fixDefeatureIssue = useAppStore((s) => s.fixDefeatureIssue);
  const fixAllDefeatureIssues = useAppStore((s) => s.fixAllDefeatureIssues);
  const selectIssue = useAppStore((s) => s.selectIssue);
  const undoLastFix = useAppStore((s) => s.undoLastFix);

  const [minFaceArea, setMinFaceArea] = useState(0.01);
  const [minEdgeLength, setMinEdgeLength] = useState(0.005);
  const [maxHoleDia, setMaxHoleDia] = useState(0.02);
  const [maxFilletR, setMaxFilletR] = useState(0.01);
  const [analyzing, setAnalyzing] = useState(false);

  const detailsRef = useRef<HTMLDivElement>(null);

  const activeIssues = defeatureIssues.filter((i) => !i.fixed);
  const fixedCount = defeatureIssues.filter((i) => i.fixed).length;

  const selectedIssue = defeatureIssues.find((i) => i.id === selectedIssueId);

  // Scroll to selected issue in the details list
  useEffect(() => {
    if (selectedIssueId && detailsRef.current) {
      const el = detailsRef.current.querySelector(`[data-issue-id="${selectedIssueId}"]`);
      if (el) el.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  }, [selectedIssueId]);

  const handleAnalyze = useCallback(() => {
    setAnalyzing(true);
    // Simulate brief analysis delay
    setTimeout(() => {
      const bodyShapes = shapes.filter(
        (s) => s.group !== 'enclosure' && s.kind !== 'enclosure'
      );
      const issues = generateIssues(bodyShapes, { minFaceArea, minEdgeLength, maxHoleDia, maxFilletR });
      setDefeatureIssues(issues);
      setAnalyzing(false);
      message.success(`Analysis complete: ${issues.length} issues found`);
    }, 400);
  }, [shapes, minFaceArea, minEdgeLength, maxHoleDia, maxFilletR, setDefeatureIssues]);

  const handleFixByKind = useCallback(
    (kind: DefeatureIssueKind) => {
      const updated = defeatureIssues.map((issue) =>
        issue.kind === kind && !issue.fixed ? { ...issue, fixed: true } : issue
      );
      setDefeatureIssues(updated);
      const count = updated.filter((i) => i.kind === kind && i.fixed).length;
      message.success(`Fixed ${count} ${issueLabels[kind].toLowerCase()}`);
    },
    [defeatureIssues, setDefeatureIssues]
  );

  const handleFixThis = useCallback(() => {
    if (!selectedIssueId) return;
    fixDefeatureIssue(selectedIssueId);
    // Auto-advance to next unfixed issue
    const currentIdx = defeatureIssues.findIndex((i) => i.id === selectedIssueId);
    const nextIssue = defeatureIssues.find((i, idx) => idx > currentIdx && !i.fixed && i.id !== selectedIssueId);
    if (nextIssue) {
      selectIssue(nextIssue.id);
    }
    message.success('Issue fixed');
  }, [selectedIssueId, defeatureIssues, fixDefeatureIssue, selectIssue]);

  const handleSkip = useCallback(() => {
    if (!selectedIssueId) return;
    const currentIdx = defeatureIssues.findIndex((i) => i.id === selectedIssueId);
    const nextIssue = defeatureIssues.find((i, idx) => idx > currentIdx && !i.fixed);
    if (nextIssue) {
      selectIssue(nextIssue.id);
    } else {
      // Wrap around
      const first = defeatureIssues.find((i) => !i.fixed);
      if (first) selectIssue(first.id);
    }
  }, [selectedIssueId, defeatureIssues, selectIssue]);

  const handleNext = useCallback(() => {
    handleSkip();
  }, [handleSkip]);

  const issueKinds: DefeatureIssueKind[] = ['small_face', 'short_edge', 'small_hole', 'sliver_face', 'gap'];

  return (
    <div style={{ padding: 12, fontSize: 12 }}>
      {/* Header */}
      <div
        style={{
          fontWeight: 600,
          marginBottom: 12,
          fontSize: 14,
          borderBottom: '1px solid #303030',
          paddingBottom: 8,
          display: 'flex',
          alignItems: 'center',
          gap: 6,
        }}
      >
        <SearchOutlined />
        Defeaturing
      </div>

      {/* Thresholds Section */}
      <div style={{ marginBottom: 12 }}>
        <div style={{ color: '#999', fontSize: 11, marginBottom: 6, fontWeight: 500, textTransform: 'uppercase', letterSpacing: 0.5 }}>
          Thresholds
        </div>
        <Form layout="vertical" size="small">
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '4px 8px' }}>
            <Form.Item label="Min Face Area" style={{ marginBottom: 4 }}>
              <InputNumber
                value={minFaceArea}
                min={0.0001}
                step={0.001}
                onChange={(v) => setMinFaceArea(v ?? 0.01)}
                style={{ width: '100%' }}
                addonAfter="m\u00B2"
                size="small"
              />
            </Form.Item>
            <Form.Item label="Min Edge Length" style={{ marginBottom: 4 }}>
              <InputNumber
                value={minEdgeLength}
                min={0.0001}
                step={0.001}
                onChange={(v) => setMinEdgeLength(v ?? 0.005)}
                style={{ width: '100%' }}
                addonAfter="m"
                size="small"
              />
            </Form.Item>
            <Form.Item label="Max Hole Dia" style={{ marginBottom: 4 }}>
              <InputNumber
                value={maxHoleDia}
                min={0.001}
                step={0.005}
                onChange={(v) => setMaxHoleDia(v ?? 0.02)}
                style={{ width: '100%' }}
                addonAfter="m"
                size="small"
              />
            </Form.Item>
            <Form.Item label="Max Fillet R" style={{ marginBottom: 4 }}>
              <InputNumber
                value={maxFilletR}
                min={0.001}
                step={0.005}
                onChange={(v) => setMaxFilletR(v ?? 0.01)}
                style={{ width: '100%' }}
                addonAfter="m"
                size="small"
              />
            </Form.Item>
          </div>
        </Form>
      </div>

      {/* Analyze Button */}
      <Button
        type="primary"
        icon={<SearchOutlined />}
        onClick={handleAnalyze}
        loading={analyzing}
        block
        size="small"
        style={{ marginBottom: 12 }}
      >
        Analyze Geometry
      </Button>

      {/* Found Issues Summary */}
      {defeatureIssues.length > 0 && (
        <>
          <Divider style={{ margin: '4px 0 8px' }} />
          <div style={{ color: '#999', fontSize: 11, marginBottom: 6, fontWeight: 500, textTransform: 'uppercase', letterSpacing: 0.5 }}>
            Found Issues
          </div>

          {/* Issue type rows */}
          <div style={{ marginBottom: 8 }}>
            {issueKinds.map((kind) => {
              const counts = countByKind(defeatureIssues, kind);
              if (counts.total === 0) return null;
              return (
                <div
                  key={kind}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    padding: '3px 0',
                    borderBottom: '1px solid #1a1a1a',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                    <span style={{ color: issueColors[kind], fontSize: 14, width: 16, textAlign: 'center' }}>
                      {issueIcons[kind]}
                    </span>
                    <span style={{ color: counts.active > 0 ? '#ddd' : '#666' }}>
                      {counts.active} {issueLabels[kind]}
                    </span>
                    {counts.active < counts.total && (
                      <span style={{ color: '#52c41a', fontSize: 10 }}>
                        ({counts.total - counts.active} fixed)
                      </span>
                    )}
                  </div>
                  {counts.active > 0 && (
                    <Button
                      type="link"
                      size="small"
                      style={{ fontSize: 11, padding: '0 4px', color: issueColors[kind] }}
                      onClick={() => handleFixByKind(kind)}
                    >
                      {issueFixLabels[kind]}
                    </Button>
                  )}
                </div>
              );
            })}
          </div>

          {/* Total and action buttons */}
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
            <span style={{ color: '#999' }}>
              Total: {activeIssues.length} active / {fixedCount} fixed
            </span>
            <Badge
              count={activeIssues.length}
              style={{ backgroundColor: activeIssues.length > 0 ? '#ff4d4f' : '#52c41a' }}
              size="small"
            />
          </div>

          <Space style={{ marginBottom: 12, width: '100%' }} direction="vertical" size={4}>
            <Button
              icon={<ThunderboltOutlined />}
              onClick={() => {
                fixAllDefeatureIssues();
                message.success('All issues fixed');
              }}
              disabled={activeIssues.length === 0}
              block
              size="small"
              style={{ background: activeIssues.length > 0 ? '#177ddc' : undefined }}
              type={activeIssues.length > 0 ? 'primary' : 'default'}
            >
              Auto Fix All
            </Button>
            <Button
              icon={<UndoOutlined />}
              onClick={() => {
                undoLastFix();
                message.info('Undid last fix');
              }}
              disabled={fixedCount === 0}
              block
              size="small"
            >
              Undo Last Fix
            </Button>
          </Space>

          {/* Issue Details - Selected issue */}
          <Divider style={{ margin: '4px 0 8px' }} />
          <div style={{ color: '#999', fontSize: 11, marginBottom: 6, fontWeight: 500, textTransform: 'uppercase', letterSpacing: 0.5 }}>
            Issue Details
          </div>

          {!selectedIssue && (
            <div style={{ color: '#666', fontSize: 11, padding: '8px 0', textAlign: 'center', border: '1px dashed #333', borderRadius: 4, marginBottom: 8 }}>
              Click an issue in 3D to select
            </div>
          )}

          {selectedIssue && (
            <div
              style={{
                background: '#1a1a2e',
                border: `1px solid ${issueColors[selectedIssue.kind]}44`,
                borderRadius: 4,
                padding: 8,
                marginBottom: 8,
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 4 }}>
                <span style={{ color: issueColors[selectedIssue.kind], fontSize: 14 }}>
                  {issueIcons[selectedIssue.kind]}
                </span>
                <Tag
                  color={issueColors[selectedIssue.kind]}
                  style={{ fontSize: 10, padding: '0 4px', lineHeight: '16px', margin: 0 }}
                >
                  {issueLabels[selectedIssue.kind]}
                </Tag>
                <span style={{ color: '#888', fontSize: 10 }}>#{selectedIssue.id}</span>
                {selectedIssue.fixed && (
                  <CheckCircleOutlined style={{ color: '#52c41a', fontSize: 12 }} />
                )}
              </div>
              <div style={{ color: '#bbb', fontSize: 11, marginBottom: 4 }}>
                {selectedIssue.description}
              </div>
              <div style={{ color: '#777', fontSize: 10, fontFamily: 'monospace' }}>
                Location: ({selectedIssue.position[0].toFixed(3)}, {selectedIssue.position[1].toFixed(3)}, {selectedIssue.position[2].toFixed(3)})
              </div>

              {!selectedIssue.fixed && (
                <div style={{ display: 'flex', gap: 4, marginTop: 8 }}>
                  <Button
                    type="primary"
                    size="small"
                    onClick={handleFixThis}
                    style={{ flex: 1, fontSize: 11 }}
                  >
                    Fix This
                  </Button>
                  <Tooltip title="Skip to next">
                    <Button
                      size="small"
                      onClick={handleSkip}
                      icon={<CloseCircleOutlined />}
                      style={{ fontSize: 11 }}
                    >
                      Skip
                    </Button>
                  </Tooltip>
                  <Tooltip title="Next issue">
                    <Button
                      size="small"
                      onClick={handleNext}
                      icon={<StepForwardOutlined />}
                      style={{ fontSize: 11 }}
                    >
                      Next
                    </Button>
                  </Tooltip>
                </div>
              )}
            </div>
          )}

          {/* Scrollable issue list */}
          <div ref={detailsRef} style={{ maxHeight: 200, overflow: 'auto' }}>
            {defeatureIssues.map((issue) => (
              <div
                key={issue.id}
                data-issue-id={issue.id}
                onClick={() => selectIssue(issue.id)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  padding: '4px 6px',
                  borderBottom: '1px solid #1a1a1a',
                  opacity: issue.fixed ? 0.4 : 1,
                  cursor: 'pointer',
                  background: issue.id === selectedIssueId ? '#1a1a3e' : 'transparent',
                  borderLeft: issue.id === selectedIssueId ? `2px solid ${issueColors[issue.kind]}` : '2px solid transparent',
                  transition: 'background 0.15s',
                }}
              >
                {issue.fixed ? (
                  <CheckCircleOutlined style={{ color: '#52c41a', fontSize: 12, flexShrink: 0 }} />
                ) : (
                  <WarningOutlined
                    style={{
                      color: issueColors[issue.kind],
                      fontSize: 12,
                      flexShrink: 0,
                    }}
                  />
                )}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                    <span style={{ color: issueColors[issue.kind], fontSize: 11, width: 12, textAlign: 'center' }}>
                      {issueIcons[issue.kind]}
                    </span>
                    <span style={{ color: '#bbb', fontSize: 10, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {issue.description}
                    </span>
                  </div>
                </div>
                {!issue.fixed && (
                  <Button
                    type="link"
                    size="small"
                    style={{ fontSize: 10, padding: 0, lineHeight: 1, flexShrink: 0 }}
                    onClick={(e) => {
                      e.stopPropagation();
                      fixDefeatureIssue(issue.id);
                      message.success('Issue fixed');
                    }}
                  >
                    Fix
                  </Button>
                )}
              </div>
            ))}
          </div>
        </>
      )}

      {defeatureIssues.length === 0 && !analyzing && (
        <div style={{ color: '#666', fontSize: 11, textAlign: 'center', padding: '16px 0' }}>
          Click "Analyze Geometry" to scan for defeaturing issues.
          <br />
          Issues will be highlighted in 3D.
        </div>
      )}
    </div>
  );
};

export default DefeaturingPanel;

import React, { useState, useCallback } from 'react';
import { Button, InputNumber, Form, Badge, Tag, Typography, Space, Divider, message } from 'antd';
import {
  BugOutlined,
  ThunderboltOutlined,
  CheckCircleOutlined,
  WarningOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';
import type { DefeatureIssueKind } from '../../store/useAppStore';

const issueColors: Record<DefeatureIssueKind, string> = {
  small_face: 'red',
  short_edge: 'orange',
  small_hole: 'gold',
  sliver_face: 'magenta',
  gap: 'cyan',
};

const issueLabels: Record<DefeatureIssueKind, string> = {
  small_face: 'Small Face',
  short_edge: 'Short Edge',
  small_hole: 'Small Hole',
  sliver_face: 'Sliver Face',
  gap: 'Gap',
};

const DefeaturingPanel: React.FC = () => {
  const defeatureIssues = useAppStore((s) => s.defeatureIssues);
  const setDefeatureIssues = useAppStore((s) => s.setDefeatureIssues);
  const fixDefeatureIssue = useAppStore((s) => s.fixDefeatureIssue);
  const fixAllDefeatureIssues = useAppStore((s) => s.fixAllDefeatureIssues);

  const [minFaceArea, setMinFaceArea] = useState(0.01);
  const [minEdgeLength, setMinEdgeLength] = useState(0.1);
  const [maxHoleDiameter, setMaxHoleDiameter] = useState(1.0);

  const activeIssues = defeatureIssues.filter((i) => !i.fixed);
  const fixedIssues = defeatureIssues.filter((i) => i.fixed);

  const handleAnalyze = useCallback(() => {
    // Generate simulated issues based on thresholds
    const issues = [];
    let id = 0;

    // Small faces
    const numSmallFaces = Math.floor(Math.random() * 4) + 1;
    for (let i = 0; i < numSmallFaces; i++) {
      const area = Math.random() * minFaceArea;
      issues.push({
        id: `df-${id++}`,
        kind: 'small_face' as const,
        description: `Face area ${area.toExponential(2)} mm^2 (< ${minFaceArea})`,
        size: area,
        fixed: false,
      });
    }

    // Short edges
    const numShortEdges = Math.floor(Math.random() * 3) + 1;
    for (let i = 0; i < numShortEdges; i++) {
      const len = Math.random() * minEdgeLength;
      issues.push({
        id: `df-${id++}`,
        kind: 'short_edge' as const,
        description: `Edge length ${len.toFixed(4)} mm (< ${minEdgeLength})`,
        size: len,
        fixed: false,
      });
    }

    // Small holes
    const numHoles = Math.floor(Math.random() * 2) + 1;
    for (let i = 0; i < numHoles; i++) {
      const dia = Math.random() * maxHoleDiameter;
      issues.push({
        id: `df-${id++}`,
        kind: 'small_hole' as const,
        description: `Hole diameter ${dia.toFixed(3)} mm (< ${maxHoleDiameter})`,
        size: dia,
        fixed: false,
      });
    }

    // Sliver faces (random)
    if (Math.random() > 0.5) {
      const ar = 20 + Math.random() * 80;
      issues.push({
        id: `df-${id++}`,
        kind: 'sliver_face' as const,
        description: `Sliver face with aspect ratio ${ar.toFixed(1)}`,
        size: ar,
        fixed: false,
      });
    }

    // Gaps (random)
    if (Math.random() > 0.4) {
      const gap = Math.random() * 0.05;
      issues.push({
        id: `df-${id++}`,
        kind: 'gap' as const,
        description: `Gap ${gap.toFixed(4)} mm between adjacent bodies`,
        size: gap,
        fixed: false,
      });
    }

    setDefeatureIssues(issues);
    message.success(`Analysis complete: ${issues.length} issues found.`);
  }, [minFaceArea, minEdgeLength, maxHoleDiameter, setDefeatureIssues]);

  return (
    <div style={{ padding: 12 }}>
      <div
        style={{
          fontWeight: 600,
          marginBottom: 12,
          fontSize: 14,
          borderBottom: '1px solid #303030',
          paddingBottom: 8,
        }}
      >
        Defeaturing
      </div>

      <Form layout="vertical" size="small">
        <Form.Item label="Min Face Area (mm^2)">
          <InputNumber
            value={minFaceArea}
            min={0.0001}
            step={0.01}
            onChange={(v) => setMinFaceArea(v ?? 0.01)}
            style={{ width: '100%' }}
          />
        </Form.Item>
        <Form.Item label="Min Edge Length (mm)">
          <InputNumber
            value={minEdgeLength}
            min={0.001}
            step={0.01}
            onChange={(v) => setMinEdgeLength(v ?? 0.1)}
            style={{ width: '100%' }}
          />
        </Form.Item>
        <Form.Item label="Max Hole Diameter (mm)">
          <InputNumber
            value={maxHoleDiameter}
            min={0.01}
            step={0.1}
            onChange={(v) => setMaxHoleDiameter(v ?? 1.0)}
            style={{ width: '100%' }}
          />
        </Form.Item>
      </Form>

      <Space style={{ marginBottom: 12 }}>
        <Button
          type="primary"
          icon={<BugOutlined />}
          onClick={handleAnalyze}
          size="small"
        >
          Analyze
        </Button>
        <Button
          icon={<ThunderboltOutlined />}
          onClick={() => {
            fixAllDefeatureIssues();
            message.success('All issues fixed.');
          }}
          disabled={activeIssues.length === 0}
          size="small"
        >
          Auto Fix All
        </Button>
      </Space>

      {defeatureIssues.length > 0 && (
        <>
          <Divider style={{ margin: '8px 0' }} />
          <div style={{ marginBottom: 8, fontSize: 12 }}>
            <Badge
              count={activeIssues.length}
              style={{ backgroundColor: activeIssues.length > 0 ? '#ff4d4f' : '#52c41a' }}
            />
            <span style={{ marginLeft: 8, color: '#999' }}>
              {activeIssues.length} active / {fixedIssues.length} fixed
            </span>
          </div>

          <div style={{ maxHeight: 300, overflow: 'auto' }}>
            {defeatureIssues.map((issue) => (
              <div
                key={issue.id}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '4px 0',
                  borderBottom: '1px solid #222',
                  opacity: issue.fixed ? 0.5 : 1,
                }}
              >
                {issue.fixed ? (
                  <CheckCircleOutlined style={{ color: '#52c41a', fontSize: 14 }} />
                ) : (
                  <WarningOutlined
                    style={{
                      color: issueColors[issue.kind],
                      fontSize: 14,
                    }}
                  />
                )}
                <div style={{ flex: 1, fontSize: 11 }}>
                  <Tag
                    color={issueColors[issue.kind]}
                    style={{ fontSize: 10, padding: '0 4px', lineHeight: '16px' }}
                  >
                    {issueLabels[issue.kind]}
                  </Tag>
                  <div style={{ color: '#bbb', marginTop: 2 }}>
                    {issue.description}
                  </div>
                </div>
                {!issue.fixed && (
                  <Button
                    type="link"
                    size="small"
                    style={{ fontSize: 11, padding: 0 }}
                    onClick={() => {
                      fixDefeatureIssue(issue.id);
                      message.success('Issue fixed.');
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

      {defeatureIssues.length === 0 && (
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          Click "Analyze" to scan geometry for defeaturing issues.
        </Typography.Text>
      )}
    </div>
  );
};

export default DefeaturingPanel;

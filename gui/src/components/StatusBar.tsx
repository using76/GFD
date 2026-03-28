import { Space, Tag, Typography } from 'antd';
import {
  PlayCircleOutlined,
  PauseCircleOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  LoadingOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../store/useAppStore';
import SelectionFilter from './SelectionFilter';

const { Text } = Typography;

export default function StatusBar() {
  const solverStatus = useAppStore((s) => s.solverStatus);
  const currentIteration = useAppStore((s) => s.currentIteration);
  const residuals = useAppStore((s) => s.residuals);
  const useGpu = useAppStore((s) => s.useGpu);
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);
  const selectedEntity = useAppStore((s) => s.selectedEntity);
  const activeTool = useAppStore((s) => s.activeTool);
  const selectionFilter = useAppStore((s) => s.selectionFilter);
  const exploded = useAppStore((s) => s.exploded);
  const transparencyMode = useAppStore((s) => s.transparencyMode);
  const repairIssues = useAppStore((s) => s.repairIssues);
  const measureLabels = useAppStore((s) => s.measureLabels);
  const measureMode = useAppStore((s) => s.measureMode);

  const statusIcon =
    solverStatus === 'running' ? (
      <LoadingOutlined style={{ color: '#52c41a' }} />
    ) : solverStatus === 'finished' ? (
      <CheckCircleOutlined style={{ color: '#1677ff' }} />
    ) : solverStatus === 'paused' ? (
      <PauseCircleOutlined style={{ color: '#faad14' }} />
    ) : (
      <PlayCircleOutlined style={{ color: '#556' }} />
    );

  const statusText =
    solverStatus === 'running'
      ? 'Solving...'
      : solverStatus === 'finished'
      ? 'Converged'
      : solverStatus === 'paused'
      ? 'Paused'
      : 'Ready';

  const lastResidual = residuals.length > 0 ? residuals[residuals.length - 1] : null;

  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        height: '100%',
        fontSize: 11,
        padding: '0 8px',
      }}
    >
      {/* Left section: status + properties */}
      <Space size="middle" style={{ flexShrink: 0 }}>
        <span>
          {statusIcon}{' '}
          <Text style={{ fontSize: 11, color: '#aab' }}>{statusText}</Text>
        </span>

        {currentIteration > 0 && (
          <Text style={{ fontSize: 11, color: '#889' }}>
            Iter: {currentIteration}
          </Text>
        )}

        {lastResidual && (
          <Text style={{ fontSize: 11, color: '#889' }}>
            Res: {lastResidual.continuity.toExponential(2)}
          </Text>
        )}

        <Text style={{ fontSize: 11, color: '#667' }}>
          Tool: {activeTool}
        </Text>

        <Text style={{ fontSize: 11, color: '#667' }}>
          Select: {selectionFilter}
        </Text>

        {exploded && (
          <Tag color="orange" style={{ fontSize: 10, lineHeight: '16px', padding: '0 4px', margin: 0 }}>
            Exploded
          </Tag>
        )}

        {transparencyMode && (
          <Tag color="blue" style={{ fontSize: 10, lineHeight: '16px', padding: '0 4px', margin: 0 }}>
            Transparent
          </Tag>
        )}

        {repairIssues.filter(i => !i.fixed).length > 0 && (
          <Tag color="orange" style={{ fontSize: 10, lineHeight: '16px', padding: '0 4px', margin: 0 }}>
            Repair: {repairIssues.filter(i => !i.fixed).length}
          </Tag>
        )}

        {measureMode && (
          <Tag color="cyan" style={{ fontSize: 10, lineHeight: '16px', padding: '0 4px', margin: 0 }}>
            Measuring
          </Tag>
        )}

        {measureLabels.length > 0 && (
          <Text style={{ fontSize: 11, color: '#889' }}>
            Meas: {measureLabels.length}
          </Text>
        )}
      </Space>

      {/* Right section: mesh info + selection filter + GPU tag */}
      <Space size="middle" style={{ flexShrink: 0 }}>
        {meshGenerated && meshDisplayData && (
          <Text style={{ fontSize: 11, color: '#889' }}>
            Cells: {meshDisplayData.cellCount.toLocaleString()} | Nodes:{' '}
            {meshDisplayData.nodeCount.toLocaleString()}
          </Text>
        )}

        {selectedEntity && (
          <Text style={{ fontSize: 11, color: '#889' }}>
            {selectedEntity.type} #{selectedEntity.id}
          </Text>
        )}

        <SelectionFilter />

        <Tag
          color={useGpu ? 'green' : 'default'}
          icon={
            useGpu ? (
              <CheckCircleOutlined />
            ) : (
              <CloseCircleOutlined />
            )
          }
          style={{ fontSize: 10, lineHeight: '16px', padding: '0 4px', margin: 0 }}
        >
          {useGpu ? 'GPU' : 'CPU'}
        </Tag>
      </Space>
    </div>
  );
}

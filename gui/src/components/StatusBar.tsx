import { Space, Tag, Typography } from 'antd';
import {
  PlayCircleOutlined,
  PauseCircleOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
  LoadingOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../store/useAppStore';

const { Text } = Typography;

export default function StatusBar() {
  const solverStatus = useAppStore((s) => s.solverStatus);
  const currentIteration = useAppStore((s) => s.currentIteration);
  const residuals = useAppStore((s) => s.residuals);
  const useGpu = useAppStore((s) => s.useGpu);
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);
  const selectedEntity = useAppStore((s) => s.selectedEntity);

  const statusIcon =
    solverStatus === 'running' ? (
      <LoadingOutlined style={{ color: '#52c41a' }} />
    ) : solverStatus === 'finished' ? (
      <CheckCircleOutlined style={{ color: '#1677ff' }} />
    ) : solverStatus === 'paused' ? (
      <PauseCircleOutlined style={{ color: '#faad14' }} />
    ) : (
      <PlayCircleOutlined style={{ color: '#888' }} />
    );

  const statusText =
    solverStatus === 'running'
      ? 'Solving...'
      : solverStatus === 'finished'
      ? 'Converged'
      : solverStatus === 'paused'
      ? 'Paused'
      : 'Idle';

  const lastResidual = residuals.length > 0 ? residuals[residuals.length - 1] : null;

  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        height: '100%',
        fontSize: 12,
      }}
    >
      <Space size="middle">
        <span>
          {statusIcon}{' '}
          <Text style={{ fontSize: 12 }}>{statusText}</Text>
        </span>

        {currentIteration > 0 && (
          <Text style={{ fontSize: 12 }}>
            Iteration: {currentIteration}
          </Text>
        )}

        {lastResidual && (
          <Text style={{ fontSize: 12 }}>
            Residual: {lastResidual.continuity.toExponential(3)}
          </Text>
        )}
      </Space>

      <Space size="middle">
        {meshGenerated && meshDisplayData && (
          <Text style={{ fontSize: 12 }}>
            Cells: {meshDisplayData.cellCount.toLocaleString()} | Nodes:{' '}
            {meshDisplayData.nodeCount.toLocaleString()}
          </Text>
        )}

        {selectedEntity && (
          <Text style={{ fontSize: 12 }}>
            Selected: {selectedEntity.type} #{selectedEntity.id}
          </Text>
        )}

        <Tag
          color={useGpu ? 'green' : 'default'}
          icon={
            useGpu ? (
              <CheckCircleOutlined />
            ) : (
              <CloseCircleOutlined />
            )
          }
          style={{ fontSize: 11, lineHeight: '18px' }}
        >
          {useGpu ? 'GPU' : 'CPU'}
        </Tag>
      </Space>
    </div>
  );
}

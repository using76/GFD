import { Space, Tag, Typography } from 'antd';
import {
  PlayCircleOutlined,
  PauseCircleOutlined,
  CheckCircleOutlined,
  CloseCircleOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../store/appStore';

const { Text } = Typography;

export default function StatusBar() {
  const solverStatus = useAppStore((s) => s.solverStatus);
  const gpuAvailable = useAppStore((s) => s.gpuAvailable);
  const meshData = useAppStore((s) => s.meshData);
  const selectedEntity = useAppStore((s) => s.selectedEntity);

  const statusIcon = solverStatus.running ? (
    <PlayCircleOutlined style={{ color: '#52c41a' }} />
  ) : solverStatus.converged ? (
    <CheckCircleOutlined style={{ color: '#1677ff' }} />
  ) : (
    <PauseCircleOutlined style={{ color: '#888' }} />
  );

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
          <Text style={{ fontSize: 12 }}>
            {solverStatus.running
              ? 'Solving...'
              : solverStatus.converged
                ? 'Converged'
                : 'Idle'}
          </Text>
        </span>

        {solverStatus.iteration > 0 && (
          <Text style={{ fontSize: 12 }}>
            Iteration: {solverStatus.iteration}
          </Text>
        )}

        {solverStatus.residual > 0 && (
          <Text style={{ fontSize: 12 }}>
            Residual: {solverStatus.residual.toExponential(3)}
          </Text>
        )}
      </Space>

      <Space size="middle">
        {meshData && (
          <Text style={{ fontSize: 12 }}>
            Cells: {meshData.cellCount.toLocaleString()} | Nodes:{' '}
            {meshData.nodeCount.toLocaleString()}
          </Text>
        )}

        {selectedEntity && (
          <Text style={{ fontSize: 12 }}>
            Selected: {selectedEntity.type} #{selectedEntity.id}
          </Text>
        )}

        <Tag
          color={gpuAvailable ? 'green' : 'default'}
          icon={
            gpuAvailable ? (
              <CheckCircleOutlined />
            ) : (
              <CloseCircleOutlined />
            )
          }
          style={{ fontSize: 11, lineHeight: '18px' }}
        >
          {gpuAvailable ? 'GPU' : 'CPU'}
        </Tag>
      </Space>
    </div>
  );
}

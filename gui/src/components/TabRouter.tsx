import { Typography, Tree, Descriptions, Empty, Button, InputNumber, Select, Space } from 'antd';
import {
  BlockOutlined,
  AppstoreOutlined,
  ControlOutlined,
  PlayCircleOutlined,
  LineChartOutlined,
} from '@ant-design/icons';
import { useAppStore, TabKey } from '../store/appStore';

const { Title, Text } = Typography;

interface TabRouterProps {
  panel: 'outline' | 'properties';
}

// ---- Outline Panel Contents (Left) ----

function CadOutline() {
  const treeData = [
    {
      title: 'Geometry',
      key: 'geometry',
      icon: <AppstoreOutlined />,
      children: [
        { title: 'Import CAD...', key: 'import-cad', isLeaf: true },
        { title: 'Create Box', key: 'create-box', isLeaf: true },
        { title: 'Create Cylinder', key: 'create-cylinder', isLeaf: true },
      ],
    },
  ];

  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>CAD</Title>
      <Tree treeData={treeData} defaultExpandAll showIcon />
    </div>
  );
}

function MeshOutline() {
  const meshData = useAppStore((s) => s.meshData);

  const treeData = [
    {
      title: 'Mesh',
      key: 'mesh-root',
      icon: <BlockOutlined />,
      children: meshData
        ? [
            { title: `Nodes (${meshData.nodeCount})`, key: 'nodes', isLeaf: true },
            { title: `Cells (${meshData.cellCount})`, key: 'cells', isLeaf: true },
            { title: `Faces (${meshData.faceCount})`, key: 'faces', isLeaf: true },
          ]
        : [{ title: 'No mesh loaded', key: 'no-mesh', isLeaf: true }],
    },
    {
      title: 'Zones',
      key: 'zones',
      children: [
        { title: 'inlet', key: 'zone-inlet', isLeaf: true },
        { title: 'outlet', key: 'zone-outlet', isLeaf: true },
        { title: 'wall', key: 'zone-wall', isLeaf: true },
      ],
    },
  ];

  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>MESH</Title>
      <Tree treeData={treeData} defaultExpandAll showIcon />
    </div>
  );
}

function SetupOutline() {
  const treeData = [
    {
      title: 'Physics',
      key: 'physics',
      icon: <ControlOutlined />,
      children: [
        { title: 'Fluid', key: 'fluid', isLeaf: true },
        { title: 'Thermal', key: 'thermal', isLeaf: true },
        { title: 'Solid', key: 'solid', isLeaf: true },
      ],
    },
    {
      title: 'Boundary Conditions',
      key: 'bc',
      children: [
        { title: 'Inlet', key: 'bc-inlet', isLeaf: true },
        { title: 'Outlet', key: 'bc-outlet', isLeaf: true },
        { title: 'Wall', key: 'bc-wall', isLeaf: true },
      ],
    },
    {
      title: 'Materials',
      key: 'materials',
      children: [
        { title: 'Air', key: 'mat-air', isLeaf: true },
        { title: 'Water', key: 'mat-water', isLeaf: true },
      ],
    },
  ];

  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>Setup</Title>
      <Tree treeData={treeData} defaultExpandAll showIcon />
    </div>
  );
}

function CalculationOutline() {
  const solverStatus = useAppStore((s) => s.solverStatus);

  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>Calculation</Title>
      <Space direction="vertical" style={{ width: '100%' }}>
        <div>
          <Text style={{ fontSize: 12 }}>Solver Algorithm</Text>
          <Select
            defaultValue="simple"
            size="small"
            style={{ width: '100%', marginTop: 4 }}
            options={[
              { value: 'simple', label: 'SIMPLE' },
              { value: 'simplec', label: 'SIMPLEC' },
              { value: 'piso', label: 'PISO' },
            ]}
          />
        </div>
        <div>
          <Text style={{ fontSize: 12 }}>Max Iterations</Text>
          <InputNumber
            defaultValue={1000}
            min={1}
            max={100000}
            size="small"
            style={{ width: '100%', marginTop: 4 }}
          />
        </div>
        <div>
          <Text style={{ fontSize: 12 }}>Convergence Criterion</Text>
          <InputNumber
            defaultValue={1e-6}
            min={1e-12}
            max={1}
            step={1e-7}
            size="small"
            style={{ width: '100%', marginTop: 4 }}
          />
        </div>
        <Button
          type="primary"
          icon={<PlayCircleOutlined />}
          block
          disabled={solverStatus.running}
          style={{ marginTop: 8 }}
        >
          {solverStatus.running ? 'Running...' : 'Run Solver'}
        </Button>
      </Space>
    </div>
  );
}

function ResultsOutline() {
  const fieldData = useAppStore((s) => s.fieldData);
  const activeField = useAppStore((s) => s.activeField);
  const setActiveField = useAppStore((s) => s.setActiveField);

  const treeData = [
    {
      title: 'Fields',
      key: 'fields',
      icon: <LineChartOutlined />,
      children:
        fieldData.length > 0
          ? fieldData.map((f) => ({
              title: f.name,
              key: `field-${f.name}`,
              isLeaf: true,
            }))
          : [{ title: 'No results', key: 'no-results', isLeaf: true }],
    },
  ];

  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>Results</Title>
      <Tree
        treeData={treeData}
        defaultExpandAll
        showIcon
        selectedKeys={activeField ? [`field-${activeField}`] : []}
        onSelect={(keys) => {
          if (keys.length > 0) {
            const key = keys[0] as string;
            if (key.startsWith('field-')) {
              setActiveField(key.replace('field-', ''));
            }
          }
        }}
      />
    </div>
  );
}

// ---- Properties Panel Contents (Right) ----

function CadProperties() {
  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>CAD Properties</Title>
      <Empty description="Select a geometry entity" />
    </div>
  );
}

function MeshProperties() {
  const meshData = useAppStore((s) => s.meshData);
  const selectedEntity = useAppStore((s) => s.selectedEntity);

  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>Mesh Properties</Title>
      {meshData ? (
        <Descriptions column={1} size="small" bordered>
          <Descriptions.Item label="Nodes">{meshData.nodeCount}</Descriptions.Item>
          <Descriptions.Item label="Cells">{meshData.cellCount}</Descriptions.Item>
          <Descriptions.Item label="Faces">{meshData.faceCount}</Descriptions.Item>
        </Descriptions>
      ) : (
        <Empty description="No mesh loaded" />
      )}
      {selectedEntity && (
        <Descriptions column={1} size="small" bordered style={{ marginTop: 12 }}>
          <Descriptions.Item label="Type">{selectedEntity.type}</Descriptions.Item>
          <Descriptions.Item label="ID">{selectedEntity.id}</Descriptions.Item>
        </Descriptions>
      )}
    </div>
  );
}

function SetupProperties() {
  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>Properties</Title>
      <Empty description="Select a setup item" />
    </div>
  );
}

function CalculationProperties() {
  const solverStatus = useAppStore((s) => s.solverStatus);

  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>Solver Status</Title>
      <Descriptions column={1} size="small" bordered>
        <Descriptions.Item label="Status">
          {solverStatus.running ? 'Running' : solverStatus.converged ? 'Converged' : 'Idle'}
        </Descriptions.Item>
        <Descriptions.Item label="Iteration">{solverStatus.iteration}</Descriptions.Item>
        <Descriptions.Item label="Residual">
          {solverStatus.residual > 0 ? solverStatus.residual.toExponential(4) : '-'}
        </Descriptions.Item>
      </Descriptions>
    </div>
  );
}

function ResultsProperties() {
  const activeField = useAppStore((s) => s.activeField);
  const fieldData = useAppStore((s) => s.fieldData);
  const field = fieldData.find((f) => f.name === activeField);

  return (
    <div style={{ padding: 8 }}>
      <Title level={5} style={{ marginBottom: 8 }}>Field Properties</Title>
      {field ? (
        <Descriptions column={1} size="small" bordered>
          <Descriptions.Item label="Name">{field.name}</Descriptions.Item>
          <Descriptions.Item label="Min">{field.min.toExponential(4)}</Descriptions.Item>
          <Descriptions.Item label="Max">{field.max.toExponential(4)}</Descriptions.Item>
          <Descriptions.Item label="Points">{field.values.length}</Descriptions.Item>
        </Descriptions>
      ) : (
        <Empty description="Select a field" />
      )}
    </div>
  );
}

// ---- Router ----

const OUTLINE_MAP: Record<TabKey, React.FC> = {
  cad: CadOutline,
  mesh: MeshOutline,
  setup: SetupOutline,
  calculation: CalculationOutline,
  results: ResultsOutline,
};

const PROPERTIES_MAP: Record<TabKey, React.FC> = {
  cad: CadProperties,
  mesh: MeshProperties,
  setup: SetupProperties,
  calculation: CalculationProperties,
  results: ResultsProperties,
};

export default function TabRouter({ panel }: TabRouterProps) {
  const activeTab = useAppStore((s) => s.activeTab);
  const map = panel === 'outline' ? OUTLINE_MAP : PROPERTIES_MAP;
  const Component = map[activeTab];
  return <Component />;
}

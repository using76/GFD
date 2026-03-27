import { useThree } from '@react-three/fiber';
import { Button, Space, Tooltip } from 'antd';
import {
  CompressOutlined,
  BorderOutlined,
  ColumnWidthOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../store/appStore';

interface ViewPreset {
  label: string;
  position: [number, number, number];
}

const VIEW_PRESETS: ViewPreset[] = [
  { label: 'Front', position: [0, 0, 8] },
  { label: 'Back', position: [0, 0, -8] },
  { label: 'Top', position: [0, 8, 0] },
  { label: 'Bottom', position: [0, -8, 0] },
  { label: 'Left', position: [-8, 0, 0] },
  { label: 'Right', position: [8, 0, 0] },
  { label: 'Iso', position: [5, 5, 5] },
];

/**
 * Inner component that uses useThree (must be inside Canvas).
 * We export it separately for potential in-canvas use.
 */
export function CameraPresetButtons() {
  const { camera } = useThree();

  const applyPreset = (preset: ViewPreset) => {
    camera.position.set(...preset.position);
    camera.lookAt(0, 0, 0);
    camera.updateProjectionMatrix();
  };

  const fitAll = () => {
    camera.position.set(5, 5, 5);
    camera.lookAt(0, 0, 0);
    camera.updateProjectionMatrix();
  };

  return { applyPreset, fitAll };
}

/**
 * Overlay camera control buttons rendered on top of the Canvas (outside R3F).
 */
export default function CameraControls() {
  const cameraMode = useAppStore((s) => s.cameraMode);
  const setCameraMode = useAppStore((s) => s.setCameraMode);
  const renderMode = useAppStore((s) => s.renderMode);
  const setRenderMode = useAppStore((s) => s.setRenderMode);

  return (
    <div
      style={{
        position: 'absolute',
        top: 8,
        left: 8,
        display: 'flex',
        flexDirection: 'column',
        gap: 6,
        zIndex: 10,
      }}
    >
      {/* View Presets */}
      <Space.Compact direction="vertical" size="small">
        {VIEW_PRESETS.map((preset) => (
          <Tooltip key={preset.label} title={preset.label} placement="right">
            <Button
              size="small"
              style={{ width: 40, fontSize: 10, padding: '0 4px' }}
            >
              {preset.label.slice(0, 3)}
            </Button>
          </Tooltip>
        ))}
      </Space.Compact>

      {/* Fit All */}
      <Tooltip title="Fit All" placement="right">
        <Button size="small" icon={<CompressOutlined />} style={{ width: 40 }} />
      </Tooltip>

      {/* Camera Toggle */}
      <Tooltip
        title={cameraMode.type === 'perspective' ? 'Orthographic' : 'Perspective'}
        placement="right"
      >
        <Button
          size="small"
          icon={<BorderOutlined />}
          style={{ width: 40 }}
          onClick={() =>
            setCameraMode({
              type: cameraMode.type === 'perspective' ? 'orthographic' : 'perspective',
            })
          }
        />
      </Tooltip>

      {/* Render Mode */}
      <Tooltip title={`Mode: ${renderMode}`} placement="right">
        <Button
          size="small"
          icon={<ColumnWidthOutlined />}
          style={{ width: 40 }}
          onClick={() => {
            const modes: Array<'wireframe' | 'solid' | 'contour'> = [
              'wireframe',
              'solid',
              'contour',
            ];
            const idx = modes.indexOf(renderMode);
            setRenderMode(modes[(idx + 1) % modes.length]);
          }}
        />
      </Tooltip>
    </div>
  );
}

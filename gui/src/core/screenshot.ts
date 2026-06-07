/**
 * Screenshot service (Phase 6) — the renderer registers a capturer; the MCP
 * `screenshot` tool calls it so an AI agent can SEE the viewport (annotated with
 * entity labels) and close the see→decide→act loop.
 *
 * Framework-agnostic: the actual WebGL capture + label projection lives in the
 * R3F renderer (src/react/engine), which registers a capturer here on mount.
 */

export interface ScreenshotLabel {
  id: string;
  name: string;
  /** Pixel coordinates of the entity's projected bbox center. */
  screenXY: [number, number];
}

export interface ScreenshotResult {
  /** PNG data URL. */
  image: string;
  labels: ScreenshotLabel[];
  width: number;
  height: number;
}

export interface ScreenshotOptions {
  width?: number;
  height?: number;
  /** Entity ids to label; default: all visible shapes. */
  labelIds?: string[];
}

export type ScreenshotCapturer = (opts: ScreenshotOptions) => Promise<ScreenshotResult>;

export class ScreenshotService {
  private capturer: ScreenshotCapturer | null = null;

  register(capturer: ScreenshotCapturer): () => void {
    this.capturer = capturer;
    return () => {
      if (this.capturer === capturer) this.capturer = null;
    };
  }

  isAvailable(): boolean {
    return this.capturer !== null;
  }

  async capture(opts: ScreenshotOptions = {}): Promise<ScreenshotResult> {
    if (!this.capturer) {
      throw new Error('No renderer registered a screenshot capturer');
    }
    return this.capturer(opts);
  }
}

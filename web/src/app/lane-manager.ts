import type { RenderCommand } from "../protocol";

export interface LaneConfig {
  id: string;
  viewType: "time-order" | "left-heavy" | "sandwich";
  profileIndex: number;
  height: number;
  scrollY: number;
  selectedFrameId?: number;
}

const HEADER_HEIGHT = 28;
const MIN_LANE_HEIGHT = 60;
const DRAG_HANDLE_SIZE = 6;

/**
 * Manages multiple vertical lanes with shared time axis.
 * Each lane displays a view of a profile.
 */
export class LaneManager {
  lanes: LaneConfig[] = [];
  private scrollX = 0;
  private zoom = 1;
  private dragState: { laneIndex: number; startY: number; startHeight: number } | null = null;

  addLane(config: Omit<LaneConfig, "scrollY"> & { scrollY?: number }): void {
    this.lanes.push({ scrollY: 0, ...config });
  }

  removeLane(id: string): void {
    this.lanes = this.lanes.filter((l) => l.id !== id);
  }

  /** Get the Y offset where a lane starts (including headers). */
  laneY(index: number): number {
    let y = 0;
    for (let i = 0; i < index; i++) {
      const lane = this.lanes[i];
      if (lane) y += HEADER_HEIGHT + lane.height;
    }
    return y;
  }

  /** Total height of all lanes. */
  totalHeight(): number {
    return this.lanes.reduce((sum, l) => sum + HEADER_HEIGHT + l.height, 0);
  }

  /** Which lane is at a given Y coordinate? Returns index or -1. */
  laneAtY(y: number): number {
    let offset = 0;
    for (let i = 0; i < this.lanes.length; i++) {
      const lane = this.lanes[i];
      if (!lane) continue;
      const laneEnd = offset + HEADER_HEIGHT + lane.height;
      if (y >= offset && y < laneEnd) return i;
      offset = laneEnd;
    }
    return -1;
  }

  /** Is the Y coordinate on a resize drag handle between lanes? */
  isOnDragHandle(y: number): number {
    let offset = 0;
    for (let i = 0; i < this.lanes.length; i++) {
      const lane = this.lanes[i];
      if (!lane) continue;
      offset += HEADER_HEIGHT + lane.height;
      if (Math.abs(y - offset) <= DRAG_HANDLE_SIZE) return i;
    }
    return -1;
  }

  /** Start dragging a lane resize handle. */
  startDrag(laneIndex: number, mouseY: number): void {
    const lane = this.lanes[laneIndex];
    if (!lane) return;
    this.dragState = { laneIndex, startY: mouseY, startHeight: lane.height };
  }

  /** Update drag position. */
  updateDrag(mouseY: number): void {
    if (!this.dragState) return;
    const lane = this.lanes[this.dragState.laneIndex];
    if (!lane) return;
    const delta = mouseY - this.dragState.startY;
    lane.height = Math.max(MIN_LANE_HEIGHT, this.dragState.startHeight + delta);
  }

  /** End dragging. */
  endDrag(): void {
    this.dragState = null;
  }

  get isDragging(): boolean {
    return this.dragState !== null;
  }

  /** Scroll the shared time axis. */
  scrollBy(dx: number, _dy: number): void {
    this.scrollX += dx;
  }

  /** Zoom in/out around a focal point. */
  zoomAt(factor: number, _focalX: number): void {
    this.zoom = Math.max(0.001, this.zoom * factor);
  }

  /** Scroll a specific lane vertically. */
  scrollLane(laneIndex: number, dy: number): void {
    const lane = this.lanes[laneIndex];
    if (lane) lane.scrollY += dy;
  }

  /** Generate lane header render commands. */
  renderHeaders(canvasWidth: number): RenderCommand[] {
    const commands: RenderCommand[] = [];
    for (let i = 0; i < this.lanes.length; i++) {
      const lane = this.lanes[i];
      if (!lane) continue;
      const y = this.laneY(i);
      commands.push({
        DrawRect: {
          rect: { x: 0, y, w: canvasWidth, h: HEADER_HEIGHT },
          color: "LaneHeaderBackground",
          border_color: "LaneBorder",
          label: null,
          frame_id: null,
        },
      });
      commands.push({
        DrawText: {
          position: { x: 8, y: y + HEADER_HEIGHT / 2 + 4 },
          text: `${lane.viewType} (profile ${lane.profileIndex})`,
          color: "LaneHeaderText",
          font_size: 12,
          align: "Left",
        },
      });
    }
    return commands;
  }

  /** Get shared transform state for the renderer. */
  getTransform(): { scrollX: number; zoom: number } {
    return { scrollX: this.scrollX, zoom: this.zoom };
  }

  get headerHeight(): number {
    return HEADER_HEIGHT;
  }
}

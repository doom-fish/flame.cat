import type { RenderCommand } from "../protocol";

export type ViewType = "time-order" | "left-heavy" | "sandwich" | "ranked";

export interface LaneConfig {
  id: string;
  viewType: ViewType;
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
 *
 * viewStart / viewEnd are fractions [0,1] of the profile duration representing
 * the visible time window.
 */
export class LaneManager {
  lanes: LaneConfig[] = [];
  /** Visible time window start as a fraction of total duration (0 = beginning). */
  viewStart = 0;
  /** Visible time window end as a fraction of total duration (1 = end). */
  viewEnd = 1;
  private dragState: null = null;

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

  /** Scroll the shared time axis by pixel delta. */
  scrollBy(dx: number, _dy: number, canvasWidth: number): void {
    const viewSpan = this.viewEnd - this.viewStart;
    // Convert pixel delta to fractional time delta
    const timeDelta = (dx / canvasWidth) * viewSpan;
    this.viewStart = Math.max(0, Math.min(1 - viewSpan, this.viewStart + timeDelta));
    this.viewEnd = this.viewStart + viewSpan;
  }

  /** Zoom in/out around a focal point (in pixels). */
  zoomAt(factor: number, focalX: number, canvasWidth: number): void {
    const viewSpan = this.viewEnd - this.viewStart;
    // Focal point as fraction of the visible window
    const focalFrac = focalX / canvasWidth;
    // Time position under the focal point
    const focalTime = this.viewStart + focalFrac * viewSpan;
    // New span after zoom
    const newSpan = Math.max(0.0001, Math.min(1, viewSpan / factor));
    // Keep the focal point stationary
    this.viewStart = Math.max(0, focalTime - focalFrac * newSpan);
    this.viewEnd = Math.min(1, this.viewStart + newSpan);
    // Re-clamp start if end hit the boundary
    if (this.viewEnd >= 1) {
      this.viewStart = Math.max(0, 1 - newSpan);
    }
  }

  /** Scroll a specific lane vertically. */
  scrollLane(laneIndex: number, dy: number): void {
    const lane = this.lanes[laneIndex];
    if (lane) lane.scrollY = Math.max(0, lane.scrollY + dy);
  }

  /** Generate lane header render commands. */
  renderHeaders(canvasWidth: number, yOffset = 0): RenderCommand[] {
    const commands: RenderCommand[] = [];
    for (let i = 0; i < this.lanes.length; i++) {
      const lane = this.lanes[i];
      if (!lane) continue;
      const y = this.laneY(i) + yOffset;
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

  /** Get the visible time window. */
  getViewWindow(): { viewStart: number; viewEnd: number } {
    return { viewStart: this.viewStart, viewEnd: this.viewEnd };
  }

  get headerHeight(): number {
    return HEADER_HEIGHT;
  }
}

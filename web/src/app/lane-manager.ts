import type { RenderCommand } from "../protocol";

export type ViewType = "time-order" | "left-heavy" | "sandwich" | "ranked";
export type TrackType = "thread" | "counter" | "marker" | "frame";

export interface LaneConfig {
  id: string;
  viewType: ViewType;
  profileIndex: number;
  height: number;
  scrollY: number;
  selectedFrameId?: number;
  /** Thread group ID within the profile (undefined = all threads). */
  threadId?: number;
  /** Display name for this lane (thread name). */
  threadName?: string;
  /** Whether this lane is visible. Hidden lanes are skipped during rendering. */
  visible: boolean;
  /** Track type — defaults to "thread" for normal flame lanes. */
  trackType?: TrackType;
  /** Counter name — only used when trackType is "counter". */
  counterName?: string;
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
  /** Global vertical scroll offset for the lane area. */
  globalScrollY = 0;

  addLane(config: Omit<LaneConfig, "scrollY" | "visible"> & { scrollY?: number; visible?: boolean }): void {
    this.lanes.push({ scrollY: 0, visible: true, ...config });
  }

  removeLane(id: string): void {
    this.lanes = this.lanes.filter((l) => l.id !== id);
  }

  /** Get visible lanes only. */
  get visibleLanes(): LaneConfig[] {
    return this.lanes.filter((l) => l.visible);
  }

  /** Get the Y offset where a visible lane starts (including headers). */
  laneY(index: number): number {
    const visible = this.visibleLanes;
    let y = 0;
    for (let i = 0; i < index; i++) {
      const lane = visible[i];
      if (lane) y += HEADER_HEIGHT + lane.height;
    }
    return y;
  }

  /** Total height of all visible lanes. */
  totalHeight(): number {
    return this.visibleLanes.reduce((sum, l) => sum + HEADER_HEIGHT + l.height, 0);
  }

  /** Which visible lane is at a given Y coordinate? Returns index into visibleLanes or -1. */
  laneAtY(y: number): number {
    const visible = this.visibleLanes;
    let offset = -this.globalScrollY;
    for (let i = 0; i < visible.length; i++) {
      const lane = visible[i];
      if (!lane) continue;
      const laneEnd = offset + HEADER_HEIGHT + lane.height;
      if (y >= offset && y < laneEnd) return i;
      offset = laneEnd;
    }
    return -1;
  }

  /** Is the Y coordinate on a resize drag handle between visible lanes? */
  isOnDragHandle(y: number): number {
    const visible = this.visibleLanes;
    let offset = -this.globalScrollY;
    for (let i = 0; i < visible.length; i++) {
      const lane = visible[i];
      if (!lane) continue;
      offset += HEADER_HEIGHT + lane.height;
      if (Math.abs(y - offset) <= DRAG_HANDLE_SIZE) return i;
    }
    return -1;
  }

  /** Move a lane from one position to another in the full lanes array. */
  moveLane(fromIndex: number, toIndex: number): void {
    if (fromIndex === toIndex) return;
    if (fromIndex < 0 || fromIndex >= this.lanes.length) return;
    if (toIndex < 0 || toIndex >= this.lanes.length) return;
    const [lane] = this.lanes.splice(fromIndex, 1);
    if (lane) this.lanes.splice(toIndex, 0, lane);
  }

  /** Convert a visible lane index to the full lanes array index. */
  visibleToFullIndex(visibleIndex: number): number {
    const visible = this.visibleLanes;
    const lane = visible[visibleIndex];
    if (!lane) return -1;
    return this.lanes.indexOf(lane);
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

  /** Scroll a specific visible lane vertically. */
  scrollLane(visibleIndex: number, dy: number): void {
    const lane = this.visibleLanes[visibleIndex];
    if (lane) lane.scrollY = Math.max(0, lane.scrollY + dy);
  }

  /** Scroll the entire lane area vertically, clamped to content bounds. */
  scrollGlobal(dy: number, viewportHeight: number): void {
    const maxScroll = Math.max(0, this.totalHeight() - viewportHeight);
    this.globalScrollY = Math.max(0, Math.min(maxScroll, this.globalScrollY + dy));
  }

  /** Generate lane header render commands for visible lanes. */
  renderHeaders(canvasWidth: number, yOffset = 0): RenderCommand[] {
    const visible = this.visibleLanes;
    const commands: RenderCommand[] = [];
    for (let i = 0; i < visible.length; i++) {
      const lane = visible[i];
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
      const label = lane.threadName ?? `${lane.viewType} (profile ${lane.profileIndex})`;
      commands.push({
        DrawText: {
          position: { x: 24, y: y + HEADER_HEIGHT / 2 + 4 },
          text: label,
          color: "LaneHeaderText",
          font_size: 12,
          align: "Left",
        },
      });
      // Drag handle icon (≡)
      commands.push({
        DrawText: {
          position: { x: 10, y: y + HEADER_HEIGHT / 2 + 4 },
          text: "≡",
          color: "LaneHeaderText",
          font_size: 14,
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

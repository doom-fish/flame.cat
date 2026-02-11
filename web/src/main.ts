import { darkTheme, lightTheme, resolveColor } from "./themes";
import type { Theme, Color } from "./themes";
import type { RenderCommand } from "./protocol";
import { WebGPURenderer } from "./renderers/webgpu";
import { CanvasRenderer } from "./renderers/canvas";
import {
  LaneManager,
  createToolbar,
  applyToolbarTheme,
  Hovertip,
  DetailPanel,
  SearchBar,
  LaneSidebar,
} from "./app";
import type { ViewType, ProfileInfo } from "./app";
import { bindInteraction } from "./app/interaction";

interface Renderer {
  render(commands: RenderCommand[], scrollX: number, scrollY: number): void;
  setTheme(theme: Theme): void;
}

async function createRenderer(
  canvas: HTMLCanvasElement,
  theme: Theme,
): Promise<{ renderer: Renderer; backend: string }> {
  if (navigator.gpu) {
    try {
      const r = new WebGPURenderer(canvas, theme);
      await r.init();
      return { renderer: r, backend: "webgpu" };
    } catch {
      console.warn("WebGPU init failed, falling back to Canvas2D");
    }
  }
  return { renderer: new CanvasRenderer(canvas, theme), backend: "canvas2d" };
}

function colorStr(c: Color): string {
  return `rgba(${Math.round(c.r * 255)},${Math.round(c.g * 255)},${Math.round(c.b * 255)},${c.a})`;
}

/** Format microseconds into a human-readable string. */
function formatTime(us: number): string {
  if (us < 1) return `${(us * 1000).toFixed(1)}ns`;
  if (us < 1000) return `${us.toFixed(1)}µs`;
  if (us < 1_000_000) return `${(us / 1000).toFixed(2)}ms`;
  return `${(us / 1_000_000).toFixed(3)}s`;
}

async function main() {
  // Layout: toolbar → canvas container → detail panel
  const root = document.createElement("div");
  root.style.cssText =
    "display:flex;flex-direction:column;width:100vw;height:100vh;position:fixed;top:0;left:0;";
  document.body.style.margin = "0";
  document.body.style.overflow = "hidden";
  document.body.style.touchAction = "none";
  document.body.appendChild(root);

  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const theme: Theme = prefersDark ? darkTheme : lightTheme;

  let activeView: ViewType = "time-order";
  let searchQuery = "";
  let profileLoaded = false;
  let profileDuration = 0;
  let selectedSpanName: string | null = null;

  // Hidden file input for file picking (supports multiple)
  const fileInput = document.createElement("input");
  fileInput.type = "file";
  fileInput.accept = ".json,.cpuprofile,.txt,.collapsed,.folded,.speedscope,.prof,.out";
  fileInput.multiple = true;
  fileInput.style.display = "none";
  document.body.appendChild(fileInput);

  const openFilePicker = () => fileInput.click();

  // Toolbar
  const toolbar = createToolbar({
    activeView,
    profileName: null,
    onViewChange: (view) => {
      activeView = view;
      switchView(view);
    },
    onSearch: () => searchBar.show(),
    onOpenFile: openFilePicker,
    onLanes: () => {
      laneSidebar.toggle();
    },
  });
  root.appendChild(toolbar);

  // Canvas container
  const canvasContainer = document.createElement("div");
  canvasContainer.style.cssText = "flex:1;position:relative;min-height:0;";
  root.appendChild(canvasContainer);

  const canvas = document.createElement("canvas");
  canvas.id = "canvas";
  canvas.style.cssText = "width:100%;height:100%;display:block;touch-action:none;";
  canvasContainer.appendChild(canvas);

  // Empty state / welcome screen
  const emptyState = document.createElement("div");
  emptyState.style.cssText = `
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    pointer-events: none;
    user-select: none;
    gap: 16px;
  `;
  const dropIcon = document.createElement("div");
  dropIcon.textContent = "🔥";
  dropIcon.style.cssText = "font-size: 48px; opacity: 0.6;";
  const dropText = document.createElement("div");
  dropText.style.cssText = `
    font-family: 'SF Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
    font-size: 14px;
    opacity: 0.5;
    text-align: center;
    line-height: 1.8;
  `;
  dropText.textContent = "Drop a profile here or click 📂 to open\nDrop multiple files or Shift+drop to align profiles\nSupports Chrome, Firefox, speedscope, pprof, collapsed, React DevTools, and more";
  const shortcutsText = document.createElement("div");
  shortcutsText.style.cssText = `
    font-family: 'SF Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
    font-size: 11px;
    opacity: 0.35;
    text-align: center;
    line-height: 1.8;
    white-space: pre-line;
  `;
  shortcutsText.textContent = "1-4: Switch views  ·  Ctrl+F: Search  ·  L: Lanes\nScroll: Pan  ·  Ctrl+Scroll: Zoom  ·  Drag: Pan";
  emptyState.appendChild(dropIcon);
  emptyState.appendChild(dropText);
  emptyState.appendChild(shortcutsText);
  canvasContainer.appendChild(emptyState);

  // Time cursor — vertical line following mouse
  const timeCursor = document.createElement("div");
  timeCursor.style.cssText = `
    position: absolute;
    top: 0;
    width: 1px;
    height: 100%;
    pointer-events: none;
    z-index: 5;
    display: none;
  `;
  const timeCursorLabel = document.createElement("div");
  timeCursorLabel.style.cssText = `
    position: absolute;
    top: 0;
    transform: translateX(4px);
    font-family: 'SF Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
    font-size: 10px;
    padding: 1px 4px;
    white-space: nowrap;
    pointer-events: none;
    border-radius: 2px;
  `;
  timeCursor.appendChild(timeCursorLabel);
  canvasContainer.appendChild(timeCursor);

  canvas.addEventListener("mousemove", (e) => {
    if (!profileLoaded) return;
    const x = e.offsetX;
    const { viewStart, viewEnd } = laneManager.getViewWindow();
    const frac = x / canvas.clientWidth;
    const timeUs = (viewStart + frac * (viewEnd - viewStart)) * profileDuration;
    timeCursor.style.left = `${x}px`;
    timeCursor.style.display = "block";
    timeCursorLabel.textContent = formatTime(timeUs);
  });
  canvas.addEventListener("mouseleave", () => {
    timeCursor.style.display = "none";
  });

  // Status bar at bottom
  const statusBar = document.createElement("div");
  statusBar.style.cssText = `
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 20px;
    display: none;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px;
    font-family: 'SF Mono', 'Menlo', 'Monaco', 'Consolas', monospace;
    font-size: 11px;
    opacity: 0.7;
    pointer-events: none;
    z-index: 10;
  `;
  const statusLeft = document.createElement("span");
  const statusRight = document.createElement("span");
  statusBar.appendChild(statusLeft);
  statusBar.appendChild(statusRight);
  canvasContainer.appendChild(statusBar);

  let _getTimeSelection: (() => { start: number; end: number } | null) | null = null;

  const updateStatusBar = () => {
    if (!profileLoaded) {
      statusBar.style.display = "none";
      return;
    }
    statusBar.style.display = "flex";
    const { viewStart, viewEnd } = laneManager.getViewWindow();
    const visibleDuration = (viewEnd - viewStart) * profileDuration;
    const timeSel = _getTimeSelection?.();
    const selInfo = timeSel ? ` · Selection: ${formatTime((timeSel.end - timeSel.start) * profileDuration)}` : "";
    statusLeft.textContent = `${formatTime(viewStart * profileDuration)} – ${formatTime(viewEnd * profileDuration)}`;
    statusRight.textContent = `Visible: ${formatTime(visibleDuration)} · ${laneManager.visibleLanes.length} lanes${selInfo}`;
  };

  // Detail panel
  const detailPanel = new DetailPanel(root);

  // Hovertip
  const hovertip = new Hovertip(canvasContainer);

  // Search bar
  const searchBar = new SearchBar(
    canvasContainer,
    (query) => {
      searchQuery = query;
      if (query && profileLoaded) {
        try {
          const firstLane = laneManager.lanes[0];
          if (firstLane) {
            const result = JSON.parse(wasm.search_spans(firstLane.profileIndex, query)) as {
              match_count: number;
              total_count: number;
            };
            searchBar.setMatchCount(result.match_count, result.total_count);
          }
        } catch {
          // ignore search errors
        }
      }
      renderAll();
    },
    () => {
      searchQuery = "";
      renderAll();
    },
  );

  const { renderer, backend } = await createRenderer(canvas, theme);
  console.log(`Using ${backend} renderer`);

  const wasm = await import("../../crates/wasm/pkg/flame_cat_wasm.js");
  await wasm.default();

  const laneManager = new LaneManager();

  // Lane sidebar
  const laneSidebar = new LaneSidebar(canvasContainer, {
    onToggle: (laneId, visible) => {
      const lane = laneManager.lanes.find((l) => l.id === laneId);
      if (lane) {
        lane.visible = visible;
        renderAll();
      }
    },
    onReorder: (from, to) => {
      laneManager.moveLane(from, to);
      laneSidebar.update(laneManager.lanes);
      renderAll();
    },
    onOffsetChange: (profileIndex, offsetUs) => {
      try {
        wasm.set_profile_offset(profileIndex, offsetUs);
        renderAll();
      } catch (err) {
        console.error("Failed to set profile offset:", err);
      }
    },
  });

  /** Refresh the profile alignment section in the sidebar. */
  const updateSidebarProfiles = () => {
    try {
      const info = JSON.parse(wasm.get_session_info()) as {
        profile_count: number;
        profiles: ProfileInfo[];
      };
      laneSidebar.updateProfiles(info.profiles);
    } catch {
      // no session yet
    }
  };

  const MINIMAP_HEIGHT = 40;
  const TIME_AXIS_HEIGHT = 24;

  const renderAll = () => {
    const allCommands: RenderCommand[] = [];
    const laneYOffset = profileLoaded ? MINIMAP_HEIGHT + TIME_AXIS_HEIGHT : 0;

    // Minimap
    if (profileLoaded && laneManager.lanes[0]) {
      try {
        const { viewStart, viewEnd } = laneManager.getViewWindow();
        const minimapJson = wasm.render_minimap(
          laneManager.lanes[0].profileIndex,
          canvas.clientWidth,
          MINIMAP_HEIGHT,
          window.devicePixelRatio,
          viewStart,
          viewEnd,
        );
        const minimapCmds: RenderCommand[] = JSON.parse(minimapJson) as RenderCommand[];
        allCommands.push(...minimapCmds);
      } catch {
        // minimap optional
      }

      // Time axis between minimap and lanes
      try {
        const firstLane = laneManager.lanes[0];
        const meta = JSON.parse(wasm.get_profile_metadata(firstLane.profileIndex)) as {
          start_time: number;
          end_time: number;
        };
        const duration = meta.end_time - meta.start_time;
        const { viewStart: vs, viewEnd: ve } = laneManager.getViewWindow();
        const relViewStart = vs * duration;
        const relViewEnd = ve * duration;
        const gridHeight = laneManager.totalHeight() + laneManager.visibleLanes.length * laneManager.headerHeight;

        allCommands.push({
          PushTransform: { translate: { x: 0, y: MINIMAP_HEIGHT }, scale: { x: 1, y: 1 } },
        });
        const axisJson = wasm.render_time_axis(
          canvas.clientWidth,
          window.devicePixelRatio,
          relViewStart,
          relViewEnd,
          gridHeight,
        );
        const axisCmds: RenderCommand[] = JSON.parse(axisJson) as RenderCommand[];
        allCommands.push(...axisCmds);
        allCommands.push("PopTransform");
      } catch {
        // time axis optional
      }
    }

    // Clip the lane area below the minimap
    const scrollOffset = -laneManager.globalScrollY;
    allCommands.push({
      SetClip: { rect: { x: 0, y: laneYOffset, w: canvas.clientWidth, h: canvas.clientHeight - laneYOffset } },
    });

    // Lane headers (offset by global scroll)
    allCommands.push(...laneManager.renderHeaders(canvas.clientWidth, laneYOffset + scrollOffset));

    // Lane content (visible lanes only)
    const { viewStart, viewEnd } = laneManager.getViewWindow();
    const visible = laneManager.visibleLanes;
    for (let i = 0; i < visible.length; i++) {
      const lane = visible[i];
      if (!lane) continue;
      const laneY = laneManager.laneY(i) + laneManager.headerHeight + laneYOffset + scrollOffset;
      try {
        // Compute absolute time window from fractional view window + profile metadata
        const meta = JSON.parse(wasm.get_profile_metadata(lane.profileIndex)) as {
          start_time: number;
          end_time: number;
        };
        const duration = meta.end_time - meta.start_time;
        const absViewStart = meta.start_time + viewStart * duration;
        const absViewEnd = meta.start_time + viewEnd * duration;

        let commandsJson: string;
        const trackType = lane.trackType ?? "thread";

        if (trackType === "counter" && lane.counterName) {
          commandsJson = wasm.render_counter(
            lane.profileIndex,
            lane.counterName,
            canvas.clientWidth,
            lane.height,
            window.devicePixelRatio,
            absViewStart,
            absViewEnd,
          );
        } else if (trackType === "marker") {
          commandsJson = wasm.render_markers(
            lane.profileIndex,
            canvas.clientWidth,
            lane.height,
            window.devicePixelRatio,
            absViewStart,
            absViewEnd,
          );
        } else if (trackType === "frame") {
          commandsJson = wasm.render_frame_track(
            lane.profileIndex,
            canvas.clientWidth,
            lane.height,
            window.devicePixelRatio,
            absViewStart,
            absViewEnd,
          );
        } else if (trackType === "async") {
          commandsJson = wasm.render_async_track(
            lane.profileIndex,
            canvas.clientWidth,
            lane.height,
            window.devicePixelRatio,
            absViewStart,
            absViewEnd,
          );
        } else {
          commandsJson = wasm.render_view(
            lane.profileIndex,
            lane.viewType,
            0,
            lane.scrollY,
            canvas.clientWidth,
            lane.height,
            window.devicePixelRatio,
            lane.selectedFrameId != null ? BigInt(lane.selectedFrameId) : undefined,
            absViewStart,
            absViewEnd,
            lane.threadId,
          );
        }

        const laneCmds: RenderCommand[] = JSON.parse(commandsJson) as RenderCommand[];

        allCommands.push({
          PushTransform: { translate: { x: 0, y: laneY }, scale: { x: 1, y: 1 } },
        });
        allCommands.push({
          SetClip: { rect: { x: 0, y: 0, w: canvas.clientWidth, h: lane.height } },
        });

        // Search: dim non-matching, highlight matching frames
        if (searchQuery) {
          const lowerQ = searchQuery.toLowerCase();
          for (const cmd of laneCmds) {
            if (
              typeof cmd !== "string" &&
              "DrawRect" in cmd &&
              cmd.DrawRect.label &&
              cmd.DrawRect.frame_id != null
            ) {
              if (cmd.DrawRect.label.toLowerCase().includes(lowerQ)) {
                // Keep original + add highlight overlay
                allCommands.push(cmd);
                allCommands.push({
                  DrawRect: {
                    rect: cmd.DrawRect.rect,
                    color: "SearchHighlight",
                    border_color: "Border",
                    label: null,
                    frame_id: null,
                  },
                });
              } else {
                // Dim non-matching
                allCommands.push({
                  DrawRect: { ...cmd.DrawRect, color: "FlameNeutral", border_color: null },
                });
              }
            } else {
              allCommands.push(cmd);
            }
          }
        } else if (selectedSpanName) {
          // Selection: highlight same-name spans, dim others
          for (const cmd of laneCmds) {
            if (
              typeof cmd !== "string" &&
              "DrawRect" in cmd &&
              cmd.DrawRect.label &&
              cmd.DrawRect.frame_id != null
            ) {
              if (cmd.DrawRect.label === selectedSpanName) {
                allCommands.push(cmd);
                allCommands.push({
                  DrawRect: {
                    rect: cmd.DrawRect.rect,
                    color: "SearchHighlight",
                    border_color: "Border",
                    label: null,
                    frame_id: null,
                  },
                });
              } else {
                allCommands.push({
                  DrawRect: { ...cmd.DrawRect, color: "FlameNeutral", border_color: null },
                });
              }
            } else {
              allCommands.push(cmd);
            }
          }
        } else {
          allCommands.push(...laneCmds);
        }

        allCommands.push("ClearClip");
        allCommands.push("PopTransform");
      } catch (err) {
        console.error(`Failed to render lane ${lane.id}:`, err);
      }
    }

    // Clear the global lane area clip
    allCommands.push("ClearClip");

    // Flow arrows (rendered above lanes, below time selection)
    if (profileLoaded) {
      try {
        const { viewStart: vs, viewEnd: ve } = laneManager.getViewWindow();
        const firstLane = laneManager.lanes[0];
        if (firstLane) {
          const meta = JSON.parse(wasm.get_profile_metadata(firstLane.profileIndex)) as {
            start_time: number;
            end_time: number;
          };
          const duration = meta.end_time - meta.start_time;
          const absVS = meta.start_time + vs * duration;
          const absVE = meta.start_time + ve * duration;
          const arrowsJson = wasm.get_flow_arrows(firstLane.profileIndex, absVS, absVE);
          const arrows = JSON.parse(arrowsJson) as {
            name: string;
            from_ts: number;
            from_tid: number;
            to_ts: number;
            to_tid: number;
          }[];

          if (arrows.length > 0) {
            // Build threadId → lane Y center mapping
            const tidToY = new Map<number, number>();
            const visible = laneManager.visibleLanes;
            for (let i = 0; i < visible.length; i++) {
              const lane = visible[i];
              if (lane?.threadId != null) {
                const ly = laneManager.laneY(i) + laneManager.headerHeight + laneYOffset + scrollOffset;
                tidToY.set(lane.threadId, ly + Math.min(lane.height, 40) / 2);
              }
            }

            const viewSpan = ve - vs;
            for (const arrow of arrows) {
              const fromY = tidToY.get(arrow.from_tid);
              const toY = tidToY.get(arrow.to_tid);
              if (fromY == null || toY == null) continue;

              const fromX = ((arrow.from_ts - absVS) / (absVE - absVS)) * canvas.clientWidth;
              const toX = ((arrow.to_ts - absVS) / (absVE - absVS)) * canvas.clientWidth;

              // Draw curved Bézier-like arrow using line segments
              const midX = (fromX + toX) / 2;
              const steps = 12;
              for (let s = 0; s < steps; s++) {
                const t0 = s / steps;
                const t1 = (s + 1) / steps;
                // Quadratic Bézier: from → (midX, midY) → to
                const cpY = (fromY + toY) / 2 - Math.abs(toY - fromY) * 0.2;
                const x0 = (1 - t0) * (1 - t0) * fromX + 2 * (1 - t0) * t0 * midX + t0 * t0 * toX;
                const y0 = (1 - t0) * (1 - t0) * fromY + 2 * (1 - t0) * t0 * cpY + t0 * t0 * toY;
                const x1 = (1 - t1) * (1 - t1) * fromX + 2 * (1 - t1) * t1 * midX + t1 * t1 * toX;
                const y1 = (1 - t1) * (1 - t1) * fromY + 2 * (1 - t1) * t1 * cpY + t1 * t1 * toY;
                allCommands.push({
                  DrawLine: { from: { x: x0, y: y0 }, to: { x: x1, y: y1 }, color: "FlowArrow", width: 1.5 },
                });
              }
              // Arrowhead at destination
              const angle = Math.atan2(toY - ((1 - 0.9) * (1 - 0.9) * fromY + 2 * (1 - 0.9) * 0.9 * ((fromY + toY) / 2 - Math.abs(toY - fromY) * 0.2) + 0.9 * 0.9 * toY),
                                       toX - ((1 - 0.9) * (1 - 0.9) * fromX + 2 * (1 - 0.9) * 0.9 * midX + 0.9 * 0.9 * toX));
              const headLen = 6;
              allCommands.push({
                DrawLine: {
                  from: { x: toX - headLen * Math.cos(angle - 0.4), y: toY - headLen * Math.sin(angle - 0.4) },
                  to: { x: toX, y: toY },
                  color: "FlowArrow",
                  width: 1.5,
                },
              });
              allCommands.push({
                DrawLine: {
                  from: { x: toX - headLen * Math.cos(angle + 0.4), y: toY - headLen * Math.sin(angle + 0.4) },
                  to: { x: toX, y: toY },
                  color: "FlowArrow",
                  width: 1.5,
                },
              });
            }
          }
        }
      } catch {
        /* flow arrows are optional */
      }
    }

    // Time selection overlay
    const timeSel = _getTimeSelection?.();
    if (timeSel && timeSel.end - timeSel.start > 0.0001) {
      const { viewStart, viewEnd } = laneManager.getViewWindow();
      const viewSpan = viewEnd - viewStart;
      const selLeft = ((timeSel.start - viewStart) / viewSpan) * canvas.clientWidth;
      const selRight = ((timeSel.end - viewStart) / viewSpan) * canvas.clientWidth;
      const selW = selRight - selLeft;
      const selTop = laneYOffset;
      const selH = canvas.clientHeight - selTop;
      // Dim areas outside selection
      if (selLeft > 0) {
        allCommands.push({
          DrawRect: { rect: { x: 0, y: selTop, w: selLeft, h: selH }, color: "FlameNeutral", border_color: null, label: null, frame_id: null },
        });
      }
      if (selRight < canvas.clientWidth) {
        allCommands.push({
          DrawRect: { rect: { x: selRight, y: selTop, w: canvas.clientWidth - selRight, h: selH }, color: "FlameNeutral", border_color: null, label: null, frame_id: null },
        });
      }
      // Selection border lines
      allCommands.push({
        DrawLine: { from: { x: selLeft, y: selTop }, to: { x: selLeft, y: selTop + selH }, color: "SearchHighlight", width: 2 },
      });
      allCommands.push({
        DrawLine: { from: { x: selRight, y: selTop }, to: { x: selRight, y: selTop + selH }, color: "SearchHighlight", width: 2 },
      });
      // Duration label on time axis
      const selDuration = (timeSel.end - timeSel.start) * profileDuration;
      const labelX = selLeft + selW / 2;
      allCommands.push({
        DrawText: { text: formatTime(selDuration), position: { x: labelX, y: selTop - 4 }, color: "SearchHighlight", font_size: 11, align: "Center" },
      });
    }

    renderer.render(allCommands, 0, 0);
    updateStatusBar();
  };

  const switchView = (view: ViewType) => {
    for (const lane of laneManager.lanes) {
      if (view === "sandwich" && !lane.selectedFrameId) {
        lane.selectedFrameId = 0;
      }
      lane.viewType = view;
    }
    updateToolbarTheme();
    renderAll();
  };

  const updateToolbarTheme = () => {
    applyToolbarTheme(
      toolbar,
      {
        bg: colorStr(resolveColor(theme, "ToolbarBackground")),
        text: colorStr(resolveColor(theme, "ToolbarText")),
        tabActive: colorStr(resolveColor(theme, "ToolbarTabActive")),
        tabHover: colorStr(resolveColor(theme, "ToolbarTabHover")),
      },
      activeView,
    );
  };

  // Hit test: find frame at mouse position
  const hitTest = (mx: number, my: number): { name: string; frameId: number; profileIndex: number } | null => {
    const laneYOffset = profileLoaded ? MINIMAP_HEIGHT + TIME_AXIS_HEIGHT : 0;    const scrollOffset = -laneManager.globalScrollY;
    const { viewStart, viewEnd } = laneManager.getViewWindow();
    const visible = laneManager.visibleLanes;
    for (let i = 0; i < visible.length; i++) {
      const lane = visible[i];
      if (!lane) continue;
      const laneY = laneManager.laneY(i) + laneManager.headerHeight + laneYOffset + scrollOffset;
      if (my < laneY || my > laneY + lane.height) continue;
      try {
        const meta = JSON.parse(wasm.get_profile_metadata(lane.profileIndex)) as {
          start_time: number;
          end_time: number;
        };
        const duration = meta.end_time - meta.start_time;
        const absViewStart = meta.start_time + viewStart * duration;
        const absViewEnd = meta.start_time + viewEnd * duration;

        const json = wasm.render_view(
          lane.profileIndex,
          lane.viewType,
          0,
          lane.scrollY,
          canvas.clientWidth,
          lane.height,
          window.devicePixelRatio,
          lane.selectedFrameId != null ? BigInt(lane.selectedFrameId) : undefined,
          absViewStart,
          absViewEnd,
          lane.threadId,
        );
        const cmds: RenderCommand[] = JSON.parse(json) as RenderCommand[];
        const localY = my - laneY;
        for (const cmd of cmds) {
          if (typeof cmd !== "string" && "DrawRect" in cmd && cmd.DrawRect.frame_id != null) {
            const r = cmd.DrawRect.rect;
            if (mx >= r.x && mx <= r.x + r.w && localY >= r.y && localY <= r.y + r.h) {
              return { name: cmd.DrawRect.label ?? "unknown", frameId: cmd.DrawRect.frame_id, profileIndex: lane.profileIndex };
            }
          }
        }
      } catch {
        /* ignore */
      }
    }
    return null;
  };

  // Hovertip on mousemove
  canvas.addEventListener("mousemove", (e) => {
    if (!profileLoaded) return;
    const result = hitTest(e.offsetX, e.offsetY);
    if (result) {
      let detail = "";
      try {
        const info = JSON.parse(wasm.get_span_info(result.profileIndex, BigInt(result.frameId))) as {
          duration: number;
          self_time: number;
          thread: string;
          category: string | null;
        };
        const pctTotal = profileDuration > 0 ? ((info.duration / profileDuration) * 100).toFixed(1) : "?";
        const pctSelf = profileDuration > 0 ? ((info.self_time / profileDuration) * 100).toFixed(1) : "?";
        detail = `Duration: ${formatTime(info.duration)} (${pctTotal}%)\nSelf: ${formatTime(info.self_time)} (${pctSelf}%)`;
        if (info.category) detail += `\nCategory: ${info.category}`;
        detail += `\nThread: ${info.thread}`;
      } catch {
        detail = `frame #${result.frameId}`;
      }
      hovertip.show(
        e.offsetX,
        e.offsetY,
        canvas.clientWidth,
        canvas.clientHeight,
        result.name,
        detail,
      );
    } else {
      hovertip.hide();
    }
  });
  canvas.addEventListener("mouseleave", () => hovertip.hide());

  // Click for selection + detail panel
  canvas.addEventListener("click", (e) => {
    if (!profileLoaded) return;
    const result = hitTest(e.offsetX, e.offsetY);
    if (result) {
      for (const lane of laneManager.lanes) lane.selectedFrameId = result.frameId;
      selectedSpanName = result.name;
      try {
        const info = JSON.parse(wasm.get_span_info(result.profileIndex, BigInt(result.frameId))) as {
          duration: number;
          self_time: number;
          thread: string;
          category: string | null;
          depth: number;
        };
        detailPanel.show(
          {
            name: result.name,
            selfTime: info.self_time,
            totalTime: info.duration,
            depth: info.depth ?? 0,
            category: info.category,
          },
          profileDuration,
        );
      } catch {
        /* ignore */
      }
      renderAll();
    } else {
      detailPanel.hide();
      selectedSpanName = null;
      for (const lane of laneManager.lanes) lane.selectedFrameId = undefined;
      renderAll();
    }
  });

  // Keyboard shortcuts
  window.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === "f") {
      e.preventDefault();
      searchBar.show();
    }
    if (e.key === "Escape" && detailPanel.isVisible) detailPanel.hide();
    if (e.key === "Escape" && laneSidebar.isVisible) laneSidebar.hide();
    if (!e.ctrlKey && !e.metaKey && !e.altKey && profileLoaded) {
      if (e.key === "l" || e.key === "L") {
        laneSidebar.toggle();
        return;
      }
      const views: Record<string, ViewType> = {
        "1": "time-order",
        "2": "left-heavy",
        "3": "sandwich",
        "4": "ranked",
      };
      const v = views[e.key];
      if (v) {
        activeView = v;
        switchView(v);
      }
    }
  });

  // Apply initial theme
  updateToolbarTheme();
  hovertip.applyTheme({
    bg: colorStr(resolveColor(theme, "Surface")),
    text: colorStr(resolveColor(theme, "TextPrimary")),
    border: colorStr(resolveColor(theme, "Border")),
  });
  detailPanel.applyTheme({
    bg: colorStr(resolveColor(theme, "ToolbarBackground")),
    text: colorStr(resolveColor(theme, "TextPrimary")),
    border: colorStr(resolveColor(theme, "Border")),
  });
  searchBar.applyTheme({
    bg: colorStr(resolveColor(theme, "Surface")),
    text: colorStr(resolveColor(theme, "TextPrimary")),
    border: colorStr(resolveColor(theme, "Border")),
    inputBg: colorStr(resolveColor(theme, "Background")),
  });
  laneSidebar.applyTheme({
    bg: colorStr(resolveColor(theme, "ToolbarBackground")),
    text: colorStr(resolveColor(theme, "TextPrimary")),
    border: colorStr(resolveColor(theme, "Border")),
  });
  // Empty state text color
  dropText.style.color = colorStr(resolveColor(theme, "TextPrimary"));
  shortcutsText.style.color = colorStr(resolveColor(theme, "TextPrimary"));
  // Status bar theme
  statusBar.style.color = colorStr(resolveColor(theme, "TextPrimary"));
  statusBar.style.background = `linear-gradient(transparent, ${colorStr(resolveColor(theme, "Background"))})`;
  // Time cursor theme
  timeCursor.style.background = colorStr(resolveColor(theme, "TextPrimary"));
  timeCursor.style.opacity = "0.3";
  timeCursorLabel.style.color = colorStr(resolveColor(theme, "TextPrimary"));
  timeCursorLabel.style.background = colorStr(resolveColor(theme, "Surface"));
  const { animateViewTo, getTimeSelection } = bindInteraction(
    canvas,
    laneManager,
    renderAll,
    () => (profileLoaded ? MINIMAP_HEIGHT + TIME_AXIS_HEIGHT : 0),
    () => profileLoaded,
    (_from, _to) => {
      laneSidebar.update(laneManager.lanes);
    },
  );
  _getTimeSelection = getTimeSelection;

  // Shared file-loading logic
  const loadFile = async (file: File, additive = false) => {
    const buffer = await file.arrayBuffer();
    const data = new Uint8Array(buffer);
    try {
      const handle = additive
        ? wasm.add_profile_with_label(data, file.name)
        : (() => {
            wasm.clear_session();
            return wasm.parse_profile(data);
          })();
      const meta = JSON.parse(wasm.get_profile_metadata(handle)) as {
        name: string | null;
        start_time: number;
        end_time: number;
      };
      const frameCount = wasm.get_frame_count(handle);
      profileDuration = meta.end_time - meta.start_time;
      profileLoaded = true;
      emptyState.style.display = "none";
      console.log(`Loaded profile: ${frameCount} frames, ${profileDuration}µs`);
      const centerEl = toolbar.querySelector("#toolbar-center");
      if (centerEl) centerEl.textContent = meta.name ?? file.name;

      if (!additive) {
        // Clear existing lanes for a fresh load
        laneManager.lanes.length = 0;
        laneManager.globalScrollY = 0;
      }

      // Create one lane per thread group in the new profile
      const threads = JSON.parse(wasm.get_thread_list(handle)) as {
        id: number;
        name: string;
        span_count: number;
        sort_key: number;
        max_depth: number;
      }[];

      const FRAME_HEIGHT = 20;
      const MIN_LANE_HEIGHT = 40;
      const MAX_LANE_HEIGHT = 400;
      const AUTO_HIDE_THRESHOLD = 3; // auto-hide threads with fewer spans

      for (const thread of threads) {
        const contentHeight = (thread.max_depth + 1) * FRAME_HEIGHT + 8;
        const laneHeight = Math.max(MIN_LANE_HEIGHT, Math.min(MAX_LANE_HEIGHT, contentHeight));
        laneManager.addLane({
          id: `thread-${handle}-${thread.id}`,
          viewType: activeView,
          profileIndex: handle,
          height: laneHeight,
          threadId: thread.id,
          threadName: `${thread.name} (${thread.span_count})`,
          visible: thread.span_count >= AUTO_HIDE_THRESHOLD,
        });
      }

      laneSidebar.update(laneManager.lanes);
      updateSidebarProfiles();

      // Add special tracks (counters, markers, frame cost)
      try {
        const extraJson = wasm.get_extra_tracks(handle);
        const extra = JSON.parse(extraJson) as {
          counter_count: number;
          marker_count: number;
          async_span_count: number;
          has_frames: boolean;
          counter_names: string[];
          marker_names: string[];
        };

        // Frame cost track (inserted at top)
        if (extra.has_frames) {
          laneManager.addLane({
            id: `frame-${handle}`,
            viewType: activeView,
            profileIndex: handle,
            height: 60,
            trackType: "frame",
            threadName: "⏱ Frame Cost",
          });
        }

        // Async spans track
        if (extra.async_span_count > 0) {
          laneManager.addLane({
            id: `async-${handle}`,
            viewType: activeView,
            profileIndex: handle,
            height: 120,
            trackType: "async",
            threadName: `🔀 Async Spans (${extra.async_span_count})`,
          });
        }

        // Counter tracks
        for (const name of extra.counter_names) {
          laneManager.addLane({
            id: `counter-${handle}-${name}`,
            viewType: activeView,
            profileIndex: handle,
            height: 60,
            trackType: "counter",
            counterName: name,
            threadName: `📊 ${name}`,
          });
        }

        // Marker track (if any marks exist)
        if (extra.marker_count > 0) {
          laneManager.addLane({
            id: `markers-${handle}`,
            viewType: activeView,
            profileIndex: handle,
            height: 40,
            trackType: "marker",
            threadName: `🔖 Markers (${extra.marker_count})`,
          });
        }

        if (extra.counter_count > 0 || extra.marker_count > 0 || extra.has_frames || extra.async_span_count > 0) {
          laneSidebar.update(laneManager.lanes);
        }
      } catch {
        // extra tracks are optional
      }

      // Update profileDuration from session info for multi-profile
      if (additive) {
        try {
          const sessionInfo = JSON.parse(wasm.get_session_info()) as {
            duration: number;
          };
          profileDuration = sessionInfo.duration;
        } catch {
          // keep single-profile duration
        }
      }

      // Zoom-to-fit: center viewport on actual content bounds
      if (!additive) {
        try {
          const bounds = JSON.parse(wasm.get_content_bounds(handle)) as { start: number; end: number };
          const contentDuration = bounds.end - bounds.start;
          if (contentDuration > 0 && profileDuration > 0 && contentDuration < profileDuration * 0.95) {
            const padding = contentDuration * 0.05;
            const fitStart = Math.max(0, (bounds.start - padding - meta.start_time) / profileDuration);
            const fitEnd = Math.min(1, (bounds.end + padding - meta.start_time) / profileDuration);
            laneManager.viewStart = fitStart;
            laneManager.viewEnd = fitEnd;
          }
        } catch {
          // fall through — keep full view
        }
      }

      renderAll();
    } catch (err) {
      console.error("Failed to load profile:", err);
    }
  };

  // File drop (desktop) — drop replaces, Shift+drop adds
  canvas.addEventListener("dragover", (e) => {
    e.preventDefault();
    e.stopPropagation();
    canvasContainer.style.outline = "2px dashed rgba(100,160,255,0.6)";
    canvasContainer.style.outlineOffset = "-4px";
  });
  canvas.addEventListener("dragleave", () => {
    canvasContainer.style.outline = "";
    canvasContainer.style.outlineOffset = "";
  });
  canvas.addEventListener("drop", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    canvasContainer.style.outline = "";
    canvasContainer.style.outlineOffset = "";
    const files = e.dataTransfer?.files;
    if (!files || files.length === 0) return;
    // First file: replace unless shift held, rest always additive
    const firstFile = files[0];
    if (firstFile) await loadFile(firstFile, e.shiftKey);
    for (let i = 1; i < files.length; i++) {
      const file = files[i];
      if (file) await loadFile(file, true);
    }
  });

  // File input — first file replaces, additional files are additive
  fileInput.addEventListener("change", async () => {
    const files = fileInput.files;
    if (!files || files.length === 0) return;
    const firstFile = files[0];
    if (firstFile) await loadFile(firstFile, false);
    for (let i = 1; i < files.length; i++) {
      const file = files[i];
      if (file) await loadFile(file, true);
    }
    fileInput.value = "";
  });

  // Resize observer — keep canvas pixel buffer matched to display size
  const resizeObserver = new ResizeObserver(() => {
    const dpr = window.devicePixelRatio;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      updateToolbarTheme();
      if (profileLoaded) renderAll();
    }
  });
  resizeObserver.observe(canvas);

  // Orientation change — re-render after layout settles
  window.addEventListener("orientationchange", () => {
    setTimeout(() => {
      updateToolbarTheme();
      if (profileLoaded) renderAll();
    }, 200);
  });

  renderer.render([], 0, 0);
  console.log("flame.cat ready — drop or open a Chrome trace JSON file to visualize");

  // Auto-load test profile in development
  if (import.meta.env.DEV) {
    try {
      const resp = await fetch("/chrome-profile.json");
      if (resp.ok) {
        const blob = await resp.blob();
        await loadFile(new File([blob], "chrome-profile.json", { type: "application/json" }));
      }
    } catch {
      // no test profile available
    }
  }
}

main().catch(console.error);

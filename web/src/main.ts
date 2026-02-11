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

  // Detail panel
  const detailPanel = new DetailPanel(root);

  // Hovertip
  const hovertip = new Hovertip(canvasContainer);

  // Search bar
  const searchBar = new SearchBar(
    canvasContainer,
    (query) => {
      searchQuery = query;
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

  const renderAll = () => {
    const allCommands: RenderCommand[] = [];
    const laneYOffset = profileLoaded ? MINIMAP_HEIGHT : 0;

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

        const commandsJson = wasm.render_view(
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
        const laneCmds: RenderCommand[] = JSON.parse(commandsJson) as RenderCommand[];

        allCommands.push({
          PushTransform: { translate: { x: 0, y: laneY }, scale: { x: 1, y: 1 } },
        });
        allCommands.push({
          SetClip: { rect: { x: 0, y: 0, w: canvas.clientWidth, h: lane.height } },
        });

        // Search: dim non-matching frames
        if (searchQuery) {
          const lowerQ = searchQuery.toLowerCase();
          for (const cmd of laneCmds) {
            if (
              typeof cmd !== "string" &&
              "DrawRect" in cmd &&
              cmd.DrawRect.label &&
              cmd.DrawRect.frame_id != null &&
              !cmd.DrawRect.label.toLowerCase().includes(lowerQ)
            ) {
              allCommands.push({
                DrawRect: { ...cmd.DrawRect, color: "FlameNeutral", border_color: null },
              });
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

    renderer.render(allCommands, 0, 0);
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
    const laneYOffset = profileLoaded ? MINIMAP_HEIGHT : 0;
    const scrollOffset = -laneManager.globalScrollY;
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
      try {
        const firstLane = laneManager.lanes[0];
        if (firstLane) {
          const json = wasm.get_ranked_entries(firstLane.profileIndex, "self", false);
          const entries = JSON.parse(json) as {
            name: string;
            self_time: number;
            total_time: number;
            count: number;
          }[];
          const entry = entries.find((e) => e.name === result.name);
          if (entry) {
            detailPanel.show(
              {
                name: entry.name,
                selfTime: entry.self_time,
                totalTime: entry.total_time,
                depth: 0,
                category: null,
              },
              profileDuration,
            );
          }
        }
      } catch {
        /* ignore */
      }
      renderAll();
    } else {
      detailPanel.hide();
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

  const { animateViewTo } = bindInteraction(
    canvas,
    laneManager,
    renderAll,
    () => (profileLoaded ? MINIMAP_HEIGHT : 0),
    () => profileLoaded,
    (_from, _to) => {
      laneSidebar.update(laneManager.lanes);
    },
  );

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
        });
      }

      laneSidebar.update(laneManager.lanes);
      updateSidebarProfiles();

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

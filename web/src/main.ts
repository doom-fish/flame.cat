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
import type { ViewType } from "./app";
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

  // Hidden file input for mobile file picking
  const fileInput = document.createElement("input");
  fileInput.type = "file";
  fileInput.accept = ".json,.cpuprofile,.txt,.collapsed,.folded,.speedscope,.prof,.out";
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
  });

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

    // Lane headers
    allCommands.push(...laneManager.renderHeaders(canvas.clientWidth, laneYOffset));

    // Lane content (visible lanes only)
    const { viewStart, viewEnd } = laneManager.getViewWindow();
    const visible = laneManager.visibleLanes;
    for (let i = 0; i < visible.length; i++) {
      const lane = visible[i];
      if (!lane) continue;
      const laneY = laneManager.laneY(i) + laneManager.headerHeight + laneYOffset;
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
  const hitTest = (mx: number, my: number): { name: string; frameId: number } | null => {
    const laneYOffset = profileLoaded ? MINIMAP_HEIGHT : 0;
    const { viewStart, viewEnd } = laneManager.getViewWindow();
    const visible = laneManager.visibleLanes;
    for (let i = 0; i < visible.length; i++) {
      const lane = visible[i];
      if (!lane) continue;
      const laneY = laneManager.laneY(i) + laneManager.headerHeight + laneYOffset;
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
              return { name: cmd.DrawRect.label ?? "unknown", frameId: cmd.DrawRect.frame_id };
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
      hovertip.show(
        e.offsetX,
        e.offsetY,
        canvas.clientWidth,
        canvas.clientHeight,
        result.name,
        `frame #${result.frameId}`,
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

  bindInteraction(
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
  const loadFile = async (file: File) => {
    const buffer = await file.arrayBuffer();
    const data = new Uint8Array(buffer);
    try {
      const handle = wasm.parse_profile(data);
      const meta = JSON.parse(wasm.get_profile_metadata(handle)) as {
        name: string | null;
        start_time: number;
        end_time: number;
      };
      const frameCount = wasm.get_frame_count(handle);
      profileDuration = meta.end_time - meta.start_time;
      profileLoaded = true;
      console.log(`Loaded profile: ${frameCount} frames, ${profileDuration}µs`);
      const centerEl = toolbar.querySelector("#toolbar-center");
      if (centerEl) centerEl.textContent = meta.name ?? file.name;

      // Clear existing lanes
      laneManager.lanes.length = 0;

      // Create one lane per thread group
      const threads = JSON.parse(wasm.get_thread_list(handle)) as {
        id: number;
        name: string;
        span_count: number;
        sort_key: number;
        max_depth: number;
      }[];

      const FRAME_HEIGHT = 20;
      const MIN_LANE_HEIGHT = 60;
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
      renderAll();
    } catch (err) {
      console.error("Failed to load profile:", err);
    }
  };

  // File drop (desktop)
  canvas.addEventListener("dragover", (e) => {
    e.preventDefault();
    e.stopPropagation();
  });
  canvas.addEventListener("drop", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    const file = e.dataTransfer?.files[0];
    if (file) await loadFile(file);
  });

  // File input (mobile + desktop fallback)
  fileInput.addEventListener("change", async () => {
    const file = fileInput.files?.[0];
    if (file) await loadFile(file);
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
}

main().catch(console.error);

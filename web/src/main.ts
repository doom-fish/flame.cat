import { darkTheme, lightTheme } from "./themes";
import type { Theme } from "./themes";
import type { RenderCommand } from "./protocol";
import { WebGPURenderer } from "./renderers/webgpu";
import { LaneManager } from "./app";
import { bindInteraction } from "./app/interaction";

async function main() {
  const canvas = document.getElementById("canvas") as HTMLCanvasElement;
  if (!canvas) throw new Error("No canvas element found");

  canvas.style.width = "100vw";
  canvas.style.height = "100vh";
  canvas.style.display = "block";
  document.body.style.margin = "0";
  document.body.style.overflow = "hidden";

  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const theme: Theme = prefersDark ? darkTheme : lightTheme;

  const renderer = new WebGPURenderer(canvas, theme);
  await renderer.init();

  const wasm = await import("../crates/wasm/pkg/flame_cat_wasm.js");
  await wasm.default();

  const laneManager = new LaneManager();

  const renderAll = () => {
    const allCommands: RenderCommand[] = [];

    // Render lane headers
    allCommands.push(...laneManager.renderHeaders(canvas.clientWidth));

    // Render each lane's content
    for (let i = 0; i < laneManager.lanes.length; i++) {
      const lane = laneManager.lanes[i];
      if (!lane) continue;
      const laneY = laneManager.laneY(i) + laneManager.headerHeight;
      try {
        const commandsJson = wasm.render_view(
          lane.profileIndex,
          lane.viewType,
          0,
          0,
          canvas.clientWidth,
          lane.height,
          window.devicePixelRatio,
          lane.selectedFrameId,
        );
        const laneCmds: RenderCommand[] = JSON.parse(commandsJson) as RenderCommand[];

        // Offset lane content by its Y position
        allCommands.push({
          PushTransform: { translate: { x: 0, y: laneY }, scale: { x: 1, y: 1 } },
        });
        allCommands.push({
          SetClip: { rect: { x: 0, y: laneY, w: canvas.clientWidth, h: lane.height } },
        });
        allCommands.push(...laneCmds);
        allCommands.push("ClearClip");
        allCommands.push("PopTransform");
      } catch (err) {
        console.error(`Failed to render lane ${lane.id}:`, err);
      }
    }

    const { scrollX } = laneManager.getTransform();
    renderer.render(allCommands, scrollX, 0);
  };

  // Bind interaction handlers
  bindInteraction(canvas, laneManager, renderAll);

  // File drop handling
  canvas.addEventListener("dragover", (e) => {
    e.preventDefault();
    e.stopPropagation();
  });

  canvas.addEventListener("drop", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    const file = e.dataTransfer?.files[0];
    if (!file) return;

    const buffer = await file.arrayBuffer();
    const data = new Uint8Array(buffer);

    try {
      const handle = wasm.parse_profile(data);
      const meta = JSON.parse(wasm.get_profile_metadata(handle)) as {
        start_time: number;
        end_time: number;
      };
      const frameCount = wasm.get_frame_count(handle);
      console.log(`Loaded profile: ${frameCount} frames, ${meta.end_time - meta.start_time}µs`);

      laneManager.addLane({
        id: `lane-${laneManager.lanes.length}`,
        viewType: "time-order",
        profileIndex: handle,
        height: Math.max(200, canvas.clientHeight / 2),
      });

      renderAll();
    } catch (err) {
      console.error("Failed to load profile:", err);
    }
  });

  // Render empty state
  renderer.render([], 0, 0);
  console.log("flame.cat ready — drop a Chrome trace JSON file to visualize");
}

main().catch(console.error);

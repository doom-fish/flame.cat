import { darkTheme, lightTheme } from "./themes";
import type { Theme } from "./themes";
import type { RenderCommand } from "./protocol";
import { WebGPURenderer } from "./renderers/webgpu";

async function main() {
  const canvas = document.getElementById("canvas") as HTMLCanvasElement;
  if (!canvas) throw new Error("No canvas element found");

  // Fill viewport
  canvas.style.width = "100vw";
  canvas.style.height = "100vh";
  canvas.style.display = "block";
  document.body.style.margin = "0";
  document.body.style.overflow = "hidden";

  // Theme selection
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const theme: Theme = prefersDark ? darkTheme : lightTheme;

  // Init WebGPU renderer
  const renderer = new WebGPURenderer(canvas, theme);
  await renderer.init();

  // Load WASM — path resolved at build time via Vite
  const wasm = await import("../crates/wasm/pkg/flame_cat_wasm.js");
  await wasm.default();

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

      const commandsJson = wasm.render_view(
        handle,
        "time-order",
        0,
        0,
        canvas.clientWidth,
        canvas.clientHeight,
        window.devicePixelRatio,
        undefined,
      );
      const commands: RenderCommand[] = JSON.parse(commandsJson) as RenderCommand[];
      renderer.render(commands, 0, 0);
    } catch (err) {
      console.error("Failed to load profile:", err);
    }
  });

  // Render empty state
  renderer.render([], 0, 0);

  console.log("flame.cat ready — drop a Chrome trace JSON file to visualize");
}

main().catch(console.error);

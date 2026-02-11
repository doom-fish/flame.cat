import { defineConfig } from "vite";
import path from "path";

export default defineConfig({
  root: ".",
  build: {
    outDir: "dist",
  },
  server: {
    fs: {
      allow: ["..", path.resolve(__dirname, "../crates/wasm/pkg")],
    },
  },
});

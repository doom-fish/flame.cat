import { defineConfig } from "vite";
import path from "path";
import basicSsl from "@vitejs/plugin-basic-ssl";

export default defineConfig({
  root: ".",
  plugins: process.env.VITE_SSL ? [basicSsl()] : [],
  build: {
    outDir: "dist",
  },
  server: {
    fs: {
      allow: ["..", path.resolve(__dirname, "../crates/wasm/pkg")],
    },
  },
});

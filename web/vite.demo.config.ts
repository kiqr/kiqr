import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { viteSingleFile } from "vite-plugin-singlefile";

// Builds one self-contained HTML file of the app for previews: open with ?demo.
export default defineConfig({
  plugins: [react(), viteSingleFile()],
  build: { outDir: "dist-demo", emptyOutDir: true, modulePreload: { polyfill: false } },
});

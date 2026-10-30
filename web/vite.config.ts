import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Dev: `npm run dev` proxies /api to the daemon on 4040.
// Build: `npm run build` writes dist/, which the daemon embeds at compile time.
export default defineConfig({
  plugins: [react()],
  server: { port: 5173, proxy: { "/api": { target: "http://127.0.0.1:4040", changeOrigin: false } } },
  build: { outDir: "dist", emptyOutDir: true },
});

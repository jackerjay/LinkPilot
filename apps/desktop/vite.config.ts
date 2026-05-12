import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri dev server proxies to this port.
const TAURI_DEV_HOST = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  // Prevent Vite from obscuring rust errors.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: TAURI_DEV_HOST || false,
    hmr: TAURI_DEV_HOST
      ? { protocol: "ws", host: TAURI_DEV_HOST, port: 1421 }
      : undefined,
    watch: {
      // Don't watch the Rust crate.
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "es2020",
    sourcemap: true,
  },
});

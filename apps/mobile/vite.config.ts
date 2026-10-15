import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// En `tauri android dev` el telefono carga la interfaz por la red: la CLI pasa la IP
// de la PC en TAURI_DEV_HOST.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1421,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1422 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: { target: "es2022", chunkSizeWarningLimit: 800 },
});

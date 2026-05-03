import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";
import { VitePWA } from "vite-plugin-pwa";

const wsProxyTarget =
  process.env.VITE_WS_PROXY_TARGET ??
  process.env.SCAD_STUDIO_WS_URL ??
  "ws://127.0.0.1:38421";

export default defineConfig({
  clearScreen: false,
  plugins: [
    wasm(),
    topLevelAwait(),
    react(),
    VitePWA({
      registerType: "autoUpdate",
      injectRegister: "auto",
      devOptions: {
        enabled: false,
      },
      workbox: {
        globPatterns: ["**/*.{js,css,html,wasm}"],
        maximumFileSizeToCacheInBytes: 4 * 1024 * 1024,
      },
      manifest: {
        name: "budn'",
        short_name: "budn",
        description: "budn' web PWA",
        theme_color: "#000000",
        background_color: "#000000",
        display: "standalone",
        start_url: "/",
        icons: [],
      },
    }),
  ],
  server: {
    port: 5173,
    host: "0.0.0.0",
    proxy: {
      "/app-server/ws": {
        target: wsProxyTarget,
        ws: true,
        changeOrigin: true,
      },
    },
    allowedHosts: true,
  },
  build: {
    target: "es2022",
    sourcemap: true,
  },
});

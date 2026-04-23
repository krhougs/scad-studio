import { defineConfig } from "vite";

// Phase 1 骨架：仅提供最小可运行 Vite 配置。
// Phase 3 会接入 React / PWA / 严格 tsconfig / 路由 / Zustand 等。
export default defineConfig({
  server: {
    port: 5173,
  },
});

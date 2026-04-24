import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "node:path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@budn/studio-web-wasm": path.resolve(
        __dirname,
        "./tests/unit/wasm-stub.ts",
      ),
    },
  },
  test: {
    environment: "jsdom",
    globals: false,
    setupFiles: ["./tests/unit/setup.ts"],
  },
  define: {
    "process.env.NODE_ENV": JSON.stringify("development"),
  },
});

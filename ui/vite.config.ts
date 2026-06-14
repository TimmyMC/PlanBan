/// <reference types="vitest/config" />
import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { "@": path.resolve(__dirname, "src") } },
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./vitest.setup.ts"],
    // Playwright owns the e2e/ specs; Vitest owns *.test.ts(x) unit/component tests.
    include: ["src/**/*.test.{ts,tsx}"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}"],
      // Entry point, type-only files, and the test/type-shim files carry no
      // testable runtime logic.
      exclude: ["src/main.tsx", "src/types.ts", "src/vite-env.d.ts", "src/vitest.d.ts"],
      // Locks in the current ~87% (App.tsx's drag/override handlers are covered
      // by Playwright e2e, not unit tests). Raise as coverage improves.
      thresholds: { lines: 85, statements: 85, functions: 82, branches: 80 },
    },
  },
});

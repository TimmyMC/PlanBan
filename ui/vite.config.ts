/// <reference types="vitest/config" />
import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { "@": path.resolve(__dirname, "src") } },
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  // Source maps in the production build so Playwright's V8 coverage (the separate
  // e2e coverage track) maps back to source lines, not the minified bundle.
  build: { sourcemap: true },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./vitest.setup.ts"],
    // Playwright owns the e2e/ specs; Vitest owns *.test.ts(x) unit/component tests.
    include: ["src/**/*.test.{ts,tsx}"],
    coverage: {
      provider: "v8",
      // text for the console summary; lcov feeds the per-PR diff-coverage gate
      // (scripts/diff-coverage.sh) and the Codecov badge.
      reporter: ["text", "lcov"],
      include: ["src/**/*.{ts,tsx}"],
      // Entry point, type-only files, and the test/type-shim files carry no
      // testable runtime logic.
      exclude: ["src/main.tsx", "src/types.ts", "src/vite-env.d.ts", "src/vitest.d.ts"],
      // This is *unit/component* coverage only. The drag/override handler logic
      // lives in useBoard and is unit-tested directly; the Tauri-only event glue
      // is `v8 ignore`d (unreachable outside the shell). E2E coverage is tracked
      // separately (ui/coverage-e2e), never mixed into this number — so this gate
      // honestly reflects what unit tests cover. Ratchet upward as it improves.
      thresholds: { lines: 90, statements: 90, functions: 90, branches: 85 },
    },
  },
});

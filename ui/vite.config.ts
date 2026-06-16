/// <reference types="vitest/config" />
import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
// Single source of truth for coverage floors, shared with the Rust gate in
// .github/workflows/ci-complete.yml. Edit the JSON, not these numbers.
import thresholds from "../.github/coverage-thresholds.json";

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
      // text for the console summary (also surfaced in the CI step summary); lcov
      // is kept for the best-effort Codecov upload (issue #30).
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
      thresholds: {
        lines: thresholds.frontend_lines,
        statements: thresholds.frontend_statements,
        functions: thresholds.frontend_functions,
        branches: thresholds.frontend_branches,
      },
    },
  },
});

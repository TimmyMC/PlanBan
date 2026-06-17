import { fileURLToPath } from "node:url";
import type MCR from "monocart-coverage-reports";

type CoverageReportOptions = NonNullable<Parameters<typeof MCR>[0]>;

// Anchor the report dir to ui/ (this file is ui/tests/), not the launch cwd —
// Playwright global setup/teardown and workers don't share a reliable cwd.
// Forward slashes: monocart mishandles Windows backslash paths in outputDir.
const uiDir = fileURLToPath(new URL("..", import.meta.url)).replace(/\\/g, "/");

// Report-only e2e (Playwright) coverage — a SEPARATE track from Vitest's unit
// coverage in ui/vite.config.ts, never merged into the unit gate. It emits lcov
// so it can later graduate to its own gate; for now CI just uploads it as an
// artifact (docs/quality-roadmap.md, Tier 2).
export const coverageOptions: CoverageReportOptions = {
  name: "Clabby e2e coverage",
  outputDir: `${uiDir}coverage-e2e`,
  reports: ["lcovonly", "console-summary"],
  // V8 page coverage includes every loaded script; keep only our app bundle and
  // the source it maps back to (build.sourcemap), dropping vendor/node_modules.
  entryFilter: (entry) => entry.url.includes("/assets/"),
  sourceFilter: (sourcePath) => /(^|\/)src\//.test(sourcePath),
};

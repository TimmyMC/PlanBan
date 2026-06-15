import { test as base, expect } from "@playwright/test";
import MCR from "monocart-coverage-reports";
import { coverageOptions } from "./coverage-options";

// Auto fixture: collect V8 coverage for every test (Chromium only) and feed it
// to monocart's on-disk cache, which is multiprocess-safe so parallel workers
// each contribute. The lcov report is generated once in global teardown. This is
// the e2e coverage track — kept entirely separate from Vitest unit coverage.
export const test = base.extend<{ autoCoverage: string }>({
  autoCoverage: [
    async ({ page, browserName }, use) => {
      const collect = browserName === "chromium";
      if (collect) {
        await page.coverage.startJSCoverage({ resetOnNavigation: false });
      }
      await use("coverage");
      if (collect) {
        const coverage = await page.coverage.stopJSCoverage();
        await MCR(coverageOptions).add(coverage);
      }
    },
    { auto: true },
  ],
});

export { expect };

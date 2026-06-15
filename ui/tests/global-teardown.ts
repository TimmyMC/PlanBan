import MCR from "monocart-coverage-reports";
import { coverageOptions } from "./coverage-options";

// Aggregate the per-test V8 coverage that the auto fixture wrote to the cache
// into ui/coverage-e2e/lcov.info (report-only; not gated).
export default async function globalTeardown() {
  await MCR(coverageOptions).generate();
}

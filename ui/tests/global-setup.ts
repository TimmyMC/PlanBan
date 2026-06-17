import MCR from "monocart-coverage-reports";
import { coverageOptions } from "./coverage-options";

// Clear any stale coverage cache before the run so the e2e report reflects only
// this run's tests.
export default function globalSetup() {
  MCR(coverageOptions).cleanCache();
}

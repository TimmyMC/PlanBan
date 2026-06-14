import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

// UI use-case coverage gate (docs/testing.md): the deterministic half of "every
// UI use case has an e2e test". Every `Api` method (enumerable from the interface)
// and every registry UX flow must have a `@usecase:` tagged Playwright spec.

const SPEC_DIR = "tests";
const specs = readdirSync(SPEC_DIR)
  .filter((f) => f.endsWith(".spec.ts"))
  .map((f) => readFileSync(path.join(SPEC_DIR, f), "utf8"))
  .join("\n");

describe("UI use-case coverage", () => {
  it("every Api method has a @usecase:api/<method> Playwright test", () => {
    const src = readFileSync("src/api/index.ts", "utf8");
    const body = src.match(/interface Api \{([\s\S]*?)\n\}/)?.[1] ?? "";
    const methods = [...body.matchAll(/^\s*(\w+)\s*\(/gm)].map((m) => m[1]);
    expect(methods.length, "could not parse the Api interface").toBeGreaterThan(0);

    const missing = methods.filter((m) => !specs.includes(`@usecase:api/${m}`));
    expect(missing, `Api methods without a tagged e2e test: ${missing.join(", ")}`).toEqual([]);
  });

  it("every registry UX flow has a @usecase:flow/<id> Playwright test", () => {
    const registry = readFileSync("../docs/use-cases.md", "utf8");
    const flows = [...registry.matchAll(/`flow\/([\w-]+)`/g)].map((m) => m[1]);
    expect(flows.length, "no flows found in docs/use-cases.md").toBeGreaterThan(0);

    const missing = flows.filter((id) => !specs.includes(`@usecase:flow/${id}`));
    expect(missing, `registry flows without a tagged test: ${missing.join(", ")}`).toEqual([]);
  });
});

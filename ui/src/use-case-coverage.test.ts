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

// Extract the `interface Api { … }` body by brace-matching, not a non-greedy
// regex. A `}` inside a return type (e.g. `Promise<{ … }>`) would let `/\n}/`
// stop early and silently drop later methods — a *false green*. Brace counting
// throws on imbalance instead, so a parse failure reddens CI loudly.
function apiInterfaceBody(src: string): string {
  const start = src.indexOf("interface Api");
  if (start === -1) throw new Error("interface Api not found in src/api/index.ts");
  const open = src.indexOf("{", start);
  if (open === -1) throw new Error("interface Api has no opening brace");
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    if (src[i] === "{") depth++;
    else if (src[i] === "}" && --depth === 0) return src.slice(open + 1, i);
  }
  throw new Error("interface Api has unbalanced braces");
}

// Method names declared at the *top level* of the interface body. Tracking brace
// depth skips identifiers nested in type literals (`Promise<{ fetched: … }>`), so
// only real `name(` signatures count.
function topLevelMethods(body: string): string[] {
  const methods: string[] = [];
  let depth = 0;
  for (const m of body.matchAll(/([A-Za-z_]\w*)\s*\(|\{|\}/g)) {
    if (m[0] === "{") depth++;
    else if (m[0] === "}") depth--;
    else if (depth === 0) methods.push(m[1]);
  }
  return methods;
}

describe("UI use-case coverage", () => {
  it("every Api method has a @usecase:api/<method> Playwright test", () => {
    const src = readFileSync("src/api/index.ts", "utf8");
    const methods = topLevelMethods(apiInterfaceBody(src));
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

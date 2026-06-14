import { describe, expect, it } from "vitest";
import { mockApi } from "@/api/mock";

describe("mockApi (the browser/Playwright data seam)", () => {
  it("returns a board with statuses, rows, and a diverged issue", async () => {
    const board = await mockApi.getBoard();
    expect(board.statuses).toContain("In Progress");
    expect(board.rows.length).toBeGreaterThan(0);
    expect(board.rows.find((r) => r.key === "PROJ-44")?.diverged).toBe(true);
  });

  it("gates an In Review -> Done move (the override demo)", async () => {
    const res = await mockApi.move("PROJ-31", "Done");
    expect(res.completed).toBe(false);
    expect(res.blocked?.error).toMatch(/not approved/i);
  });

  it("completes a non-gated move", async () => {
    const res = await mockApi.move("PROJ-58", "In Progress");
    expect(res.completed).toBe(true);
  });

  it("override resolves a blocked issue", async () => {
    const res = await mockApi.override("PROJ-31", "overridden");
    expect(res.completed).toBe(true);
  });
});

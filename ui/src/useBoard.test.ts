import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "@/api";
import { useBoard } from "@/useBoard";

// Spy on the (mock) api per test so the mock's shared mutable rows can't leak
// state across tests — each test pins exactly the responses it exercises.
afterEach(() => {
  vi.restoreAllMocks();
});

async function mounted() {
  const view = renderHook(() => useBoard());
  await waitFor(() => expect(view.result.current.data).not.toBeNull());
  return view;
}

describe("useBoard", () => {
  it("loads the board from the api on mount", async () => {
    const getBoard = vi.spyOn(api, "getBoard");
    const { result } = await mounted();
    expect(getBoard).toHaveBeenCalledTimes(1);
    expect(result.current.data?.rows.some((r) => r.key === "PROJ-12")).toBe(true);
  });

  it("onMove completes a move and leaves the banner clear", async () => {
    vi.spyOn(api, "move").mockResolvedValue({ completed: true });
    const { result } = await mounted();

    await act(async () => {
      await result.current.onMove("PROJ-58", "In Progress");
    });

    expect(api.move).toHaveBeenCalledWith("PROJ-58", "In Progress");
    expect(result.current.banner).toBeNull();
  });

  it("onMove surfaces a banner when the move is blocked", async () => {
    vi.spyOn(api, "move").mockResolvedValue({
      completed: false,
      blocked: { stepId: "require_review", error: "PR is not approved yet" },
    });
    const { result } = await mounted();

    await act(async () => {
      await result.current.onMove("PROJ-31", "Done");
    });

    expect(result.current.banner).toEqual({
      key: "PROJ-31",
      text: "Blocked moving PROJ-31 → Done: PR is not approved yet.",
    });
  });

  it("onOverride is a no-op when there is no banner", async () => {
    const override = vi.spyOn(api, "override");
    const { result } = await mounted();

    await act(async () => {
      await result.current.onOverride();
    });

    expect(override).not.toHaveBeenCalled();
    expect(result.current.banner).toBeNull();
  });

  it("onOverride resolves the blocked move and clears the banner", async () => {
    vi.spyOn(api, "move").mockResolvedValue({
      completed: false,
      blocked: { stepId: "require_review", error: "PR is not approved yet" },
    });
    const override = vi.spyOn(api, "override").mockResolvedValue({ completed: true });
    const { result } = await mounted();

    await act(async () => {
      await result.current.onMove("PROJ-31", "Done");
    });
    expect(result.current.banner).not.toBeNull();

    await act(async () => {
      await result.current.onOverride();
    });

    expect(override).toHaveBeenCalledWith("PROJ-31", "overridden from the board");
    expect(result.current.banner).toBeNull();
  });

  it("sync triggers a tracker sync and re-fetches the board", async () => {
    const sync = vi.spyOn(api, "sync");
    const getBoard = vi.spyOn(api, "getBoard");
    const { result } = await mounted();
    getBoard.mockClear(); // ignore the mount fetch; assert the sync-driven one

    await act(async () => {
      await result.current.sync();
    });

    expect(sync).toHaveBeenCalledTimes(1);
    expect(getBoard).toHaveBeenCalledTimes(1);
  });
});

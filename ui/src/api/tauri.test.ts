import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { tauriApi } from "@/api/tauri";

describe("tauriApi", () => {
  beforeEach(() => invoke.mockReset());

  it("maps each method to the right invoke command and args", () => {
    tauriApi.getBoard();
    expect(invoke).toHaveBeenLastCalledWith("get_board");

    tauriApi.sync();
    expect(invoke).toHaveBeenLastCalledWith("sync");

    tauriApi.move("PROJ-1", "Done");
    expect(invoke).toHaveBeenLastCalledWith("move_issue", { key: "PROJ-1", to: "Done" });

    tauriApi.override("PROJ-1", "because");
    expect(invoke).toHaveBeenLastCalledWith("override_issue", { key: "PROJ-1", reason: "because" });
  });
});

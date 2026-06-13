// Tauri-backed Api implementation — used when running inside the desktop shell.
// Swapped in by `index.ts` when `window.__TAURI_INTERNALS__` is present.
// In browser / Playwright the mock is used instead; this file is still bundled
// but its `invoke` calls are never reached (guarded by `isTauri` in index.ts).
import { invoke } from "@tauri-apps/api/core";
import type { BoardData, MoveResult } from "@/types";

export const tauriApi = {
  getBoard: (): Promise<BoardData> => invoke("get_board"),

  sync: (): Promise<{ fetched: number; diverged: number }> => invoke("sync"),

  move: (key: string, to: string): Promise<MoveResult> =>
    invoke("move_issue", { key, to }),

  override: (key: string, reason: string): Promise<MoveResult> =>
    invoke("override_issue", { key, reason }),
};

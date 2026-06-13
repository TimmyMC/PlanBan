import { mockApi } from "@/api/mock";
import { tauriApi } from "@/api/tauri";
import type { BoardData, MoveResult } from "@/types";

/// The board's data port. Tauri shell uses `tauriApi` (invoke-backed);
/// browser and Playwright use `mockApi`. Core is the source of truth —
/// this is just the driver boundary (§7).
export interface Api {
  getBoard(): Promise<BoardData>;
  sync(): Promise<{ fetched: number; diverged: number }>;
  move(key: string, to: string): Promise<MoveResult>;
  override(key: string, reason: string): Promise<MoveResult>;
}

// True when running inside the Tauri desktop shell.
export const isTauri = Boolean((window as unknown as Record<string, unknown>).__TAURI_INTERNALS__);

export const api: Api = isTauri ? tauriApi : mockApi;

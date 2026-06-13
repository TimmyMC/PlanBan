import type { BoardData, MoveResult } from "@/types";
import { mockApi } from "@/api/mock";

/// The board's data port. The Tauri shell swaps in an `invoke()`-backed
/// implementation; in the browser / Playwright the mock provides seeded data.
/// Core stays the source of truth — this is just the driver boundary (§7).
export interface Api {
  getBoard(): Promise<BoardData>;
  sync(): Promise<{ fetched: number; diverged: number }>;
  move(key: string, to: string): Promise<MoveResult>;
  override(key: string, reason: string): Promise<MoveResult>;
}

// TODO(M3 wiring): when running under Tauri (window.__TAURI__), use the
// invoke-backed Api instead of the mock.
export const api: Api = mockApi;

import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { api, isTauri } from "@/api";
import type { BoardData } from "@/types";

export type Banner = { key: string; text: string };

export type BoardController = {
  data: BoardData | null;
  banner: Banner | null;
  refresh: () => Promise<void>;
  sync: () => Promise<void>;
  onMove: (key: string, to: string) => Promise<void>;
  onOverride: () => Promise<void>;
};

/// The board's state + actions, lifted out of the view so the branching logic
/// (blocked-move banner, override guard) is unit-testable against the mock api
/// without driving a real dnd-kit drag. The drag *gesture* stays an e2e concern;
/// this hook owns the *logic* the gesture triggers.
export function useBoard(): BoardController {
  const [data, setData] = useState<BoardData | null>(null);
  const [banner, setBanner] = useState<Banner | null>(null);

  const refresh = useCallback(async () => {
    setData(await api.getBoard());
  }, []);

  const sync = useCallback(async () => {
    await api.sync();
    await refresh();
  }, [refresh]);

  const onMove = useCallback(
    async (key: string, to: string) => {
      const res = await api.move(key, to);
      if (!res.completed && res.blocked) {
        setBanner({ key, text: `Blocked moving ${key} → ${to}: ${res.blocked.error}.` });
      } else {
        setBanner(null);
      }
      await refresh();
    },
    [refresh],
  );

  const onOverride = useCallback(async () => {
    if (!banner) return;
    await api.override(banner.key, "overridden from the board");
    setBanner(null);
    await refresh();
  }, [banner, refresh]);

  // Load the board once on mount.
  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Subscribe to real-time push events from the Rust EventBus (§3, §8).
  // Guarded by `isTauri` so the `listen` call is never reached in browser /
  // Playwright — which is also why this effect is excluded from unit coverage:
  // no jsdom or browser test can run inside the Tauri shell to reach it.
  /* v8 ignore start */
  useEffect(() => {
    if (!isTauri) return;
    const p = listen("clabby://event", () => {
      void refresh();
    });
    return () => {
      void p.then((f) => f());
    };
  }, [refresh]);
  /* v8 ignore stop */

  return { data, banner, refresh, sync, onMove, onOverride };
}

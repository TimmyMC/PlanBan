import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, isTauri } from "@/api";
import { Board } from "@/components/Board";
import { Button } from "@/components/ui/button";
import type { BoardData } from "@/types";

export default function App() {
  const [data, setData] = useState<BoardData | null>(null);
  const [banner, setBanner] = useState<{ key: string; text: string } | null>(null);

  async function refresh() {
    setData(await api.getBoard());
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: load the board once on mount.
  useEffect(() => {
    void refresh();
  }, []);

  // Subscribe to real-time push events from the Rust EventBus (§3, §8).
  // Guarded by `isTauri` so the `listen` call is never reached in browser / Playwright.
  // biome-ignore lint/correctness/useExhaustiveDependencies: subscribe once on mount.
  useEffect(() => {
    if (!isTauri) return;
    const p = listen("clabby://event", () => {
      void refresh();
    });
    return () => {
      void p.then((f) => f());
    };
  }, []);

  async function onMove(key: string, to: string) {
    const res = await api.move(key, to);
    if (!res.completed && res.blocked) {
      setBanner({ key, text: `Blocked moving ${key} → ${to}: ${res.blocked.error}.` });
    } else {
      setBanner(null);
    }
    await refresh();
  }

  async function onOverride() {
    if (!banner) return;
    await api.override(banner.key, "overridden from the board");
    setBanner(null);
    await refresh();
  }

  if (!data) {
    return <div className="p-8 text-muted-foreground">Loading…</div>;
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <h1 className="text-lg font-semibold">
          Clabby <span className="text-sm font-normal text-muted-foreground">command center</span>
        </h1>
        <Button
          onClick={async () => {
            await api.sync();
            await refresh();
          }}
        >
          Sync
        </Button>
      </header>

      {banner && (
        <div
          data-testid="banner"
          className="flex items-center justify-between gap-4 border-b border-destructive/40 bg-destructive/20 px-4 py-2 text-sm"
        >
          <span>{banner.text}</span>
          <button
            type="button"
            data-testid="override"
            onClick={onOverride}
            className="rounded bg-destructive px-2 py-1 text-xs font-medium text-foreground"
          >
            Override &amp; resume
          </button>
        </div>
      )}

      <main className="flex-1 overflow-auto">
        <Board data={data} onMove={onMove} />
      </main>
    </div>
  );
}

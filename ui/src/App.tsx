import { Board } from "@/components/Board";
import { Button } from "@/components/ui/button";
import { useBoard } from "@/useBoard";

export default function App() {
  const { data, banner, sync, onMove, onOverride } = useBoard();

  if (!data) {
    return <div className="p-8 text-muted-foreground">Loading…</div>;
  }

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <h1 className="text-lg font-semibold">
          Clabby <span className="text-sm font-normal text-muted-foreground">command center</span>
        </h1>
        <Button onClick={sync}>Sync</Button>
      </header>

      {banner && (
        <section
          data-testid="banner"
          // A named <section> is implicitly a `region` landmark: keeps the
          // banner's text inside a landmark (axe `region`) and announces the
          // blocked move to screen readers when it appears.
          aria-label="Blocked move"
          aria-live="assertive"
          className="flex items-center justify-between gap-4 border-b border-destructive/40 bg-destructive/20 px-4 py-2 text-sm"
        >
          <span>{banner.text}</span>
          <button
            type="button"
            data-testid="override"
            onClick={onOverride}
            // Light text on the solid `bg-destructive` red — `text-foreground`
            // here is near-black (2.41:1, fails WCAG AA); the axe banner-state
            // scan guards this.
            className="rounded bg-destructive px-2 py-1 text-xs font-medium text-primary-foreground"
          >
            Override &amp; resume
          </button>
        </section>
      )}

      <main className="flex-1 overflow-auto">
        <Board data={data} onMove={onMove} />
      </main>
    </div>
  );
}

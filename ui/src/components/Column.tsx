import { useDroppable } from "@dnd-kit/core";
import { IssueCard } from "@/components/IssueCard";
import { cn } from "@/lib/utils";
import type { OverviewRow } from "@/types";

export function Column({ status, rows }: { status: string; rows: OverviewRow[] }) {
  const { setNodeRef, isOver } = useDroppable({ id: status });

  return (
    <div
      ref={setNodeRef}
      data-testid={`col-${status}`}
      className={cn(
        "flex w-72 shrink-0 flex-col rounded-xl bg-muted/30 p-2",
        isOver && "ring-2 ring-accent",
      )}
    >
      <div className="flex items-center justify-between px-2 py-1">
        <h2 className="text-sm font-semibold">{status}</h2>
        <span className="text-xs text-muted-foreground">{rows.length}</span>
      </div>
      <div className="flex min-h-16 flex-col gap-2 p-1">
        {rows.map((r) => (
          <IssueCard key={r.key} row={r} />
        ))}
      </div>
    </div>
  );
}

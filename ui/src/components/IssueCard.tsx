import { useDraggable } from "@dnd-kit/core";
import type { OverviewRow } from "@/types";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export function IssueCard({ row }: { row: OverviewRow }) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: row.key,
  });
  const style = transform
    ? { transform: `translate3d(${transform.x}px, ${transform.y}px, 0)` }
    : undefined;

  return (
    <div
      ref={setNodeRef}
      style={style}
      data-testid={`card-${row.key}`}
      className={cn(
        "cursor-grab touch-none rounded-lg border border-border bg-card p-3 shadow-sm",
        "hover:border-accent/50",
        isDragging && "opacity-50",
      )}
      {...listeners}
      {...attributes}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-xs text-muted-foreground">{row.key}</span>
        {row.diverged && <Badge variant="destructive">diverged → {row.trackerStatus}</Badge>}
      </div>

      <div className="mt-1 text-sm leading-snug">{row.summary}</div>

      <div className="mt-2 flex flex-wrap gap-1">
        {row.sessions.map((s) => (
          <Badge key={s.id} variant="accent">
            ● {s.kind} {s.status}
          </Badge>
        ))}
        {row.git?.dirty && <Badge>dirty</Badge>}
        {row.git && row.git.ahead > 0 && <Badge>+{row.git.ahead} ahead</Badge>}
        {row.worktreePath && <Badge>{row.worktreePath}</Badge>}
      </div>
    </div>
  );
}

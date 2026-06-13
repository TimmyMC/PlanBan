import { DndContext, type DragEndEvent, PointerSensor, useSensor, useSensors } from "@dnd-kit/core";
import { Column } from "@/components/Column";
import type { BoardData } from "@/types";

export function Board({
  data,
  onMove,
}: {
  data: BoardData;
  onMove: (key: string, to: string) => void;
}) {
  // A small activation distance so a click doesn't start a drag.
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  function handleDragEnd(event: DragEndEvent) {
    const key = String(event.active.id);
    const to = event.over ? String(event.over.id) : undefined;
    const current = data.rows.find((r) => r.key === key)?.status;
    if (to && to !== current) onMove(key, to);
  }

  return (
    <DndContext sensors={sensors} onDragEnd={handleDragEnd}>
      <div className="flex gap-3 overflow-x-auto p-4">
        {data.statuses.map((s) => (
          <Column key={s} status={s} rows={data.rows.filter((r) => r.status === s)} />
        ))}
      </div>
    </DndContext>
  );
}

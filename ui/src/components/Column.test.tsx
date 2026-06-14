import { DndContext } from "@dnd-kit/core";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Column } from "@/components/Column";
import type { OverviewRow } from "@/types";

const row = (key: string, status: string): OverviewRow => ({
  key,
  summary: `summary ${key}`,
  status,
  trackerStatus: status,
  diverged: false,
  sessions: [],
  worktreePath: null,
  git: null,
});

const wrap = (status: string, rows: OverviewRow[]) => (
  <DndContext>
    <Column status={status} rows={rows} />
  </DndContext>
);

describe("Column", () => {
  it("renders the status heading, the count, and a card per row", () => {
    render(wrap("In Progress", [row("PROJ-1", "In Progress"), row("PROJ-2", "In Progress")]));
    const col = screen.getByTestId("col-In Progress");
    expect(col).toHaveTextContent("In Progress");
    expect(col).toHaveTextContent("2");
    expect(screen.getByTestId("card-PROJ-1")).toBeInTheDocument();
    expect(screen.getByTestId("card-PROJ-2")).toBeInTheDocument();
  });

  it("shows a zero count for an empty column", () => {
    render(wrap("Done", []));
    expect(screen.getByTestId("col-Done")).toHaveTextContent("0");
  });
});

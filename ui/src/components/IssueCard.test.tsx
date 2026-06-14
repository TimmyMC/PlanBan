import { DndContext } from "@dnd-kit/core";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { IssueCard } from "@/components/IssueCard";
import type { OverviewRow } from "@/types";

const row: OverviewRow = {
  key: "PROJ-12",
  summary: "Implement tracker sync",
  status: "In Progress",
  trackerStatus: "In Progress",
  diverged: false,
  sessions: [{ id: 1, kind: "managed", status: "running" }],
  worktreePath: "wt/PROJ-12",
  git: { branch: "f", ahead: 3, behind: 0, dirty: true, lastCommitSummary: "wip" },
};

// useDraggable requires a DndContext ancestor.
const wrap = (r: OverviewRow) => (
  <DndContext>
    <IssueCard row={r} />
  </DndContext>
);

describe("IssueCard", () => {
  it("surfaces the issue's key, summary, session, and git state", () => {
    render(wrap(row));
    expect(screen.getByText("PROJ-12")).toBeInTheDocument();
    expect(screen.getByText("Implement tracker sync")).toBeInTheDocument();
    expect(screen.getByText(/managed running/)).toBeInTheDocument();
    expect(screen.getByText("dirty")).toBeInTheDocument();
    expect(screen.getByText("+3 ahead")).toBeInTheDocument();
    expect(screen.getByText("wt/PROJ-12")).toBeInTheDocument();
  });

  it("shows the divergence badge only when diverged", () => {
    const { rerender } = render(wrap(row));
    expect(screen.queryByText(/diverged/)).toBeNull();
    rerender(wrap({ ...row, diverged: true, trackerStatus: "Done" }));
    expect(screen.getByText(/diverged/)).toBeInTheDocument();
  });
});

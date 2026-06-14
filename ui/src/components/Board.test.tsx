import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Board } from "@/components/Board";
import type { BoardData } from "@/types";

const data: BoardData = {
  statuses: ["To Do", "In Progress", "Done"],
  rows: [
    {
      key: "PROJ-1",
      summary: "first",
      status: "To Do",
      trackerStatus: "To Do",
      diverged: false,
      sessions: [],
      worktreePath: null,
      git: null,
    },
  ],
};

describe("Board", () => {
  it("renders a column per status and places cards in their status column", () => {
    render(<Board data={data} onMove={vi.fn()} />);
    for (const s of data.statuses) {
      expect(screen.getByTestId(`col-${s}`)).toBeInTheDocument();
    }
    expect(screen.getByTestId("col-To Do")).toHaveTextContent("PROJ-1");
    // Empty columns still render.
    expect(screen.getByTestId("col-Done")).toHaveTextContent("0");
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import App from "@/App";
import type { BoardController } from "@/useBoard";

// App's banner + loading branches are pure presentation driven by useBoard's
// state. The hook's *logic* is tested in useBoard.test.ts; here we mock the hook
// to render App in each state and assert the view wires up correctly — no drag
// gesture needed (that stays an e2e concern).
const controller = vi.hoisted(() => ({ current: null as unknown as BoardController }));
vi.mock("@/useBoard", () => ({ useBoard: () => controller.current }));

function setController(partial: Partial<BoardController>) {
  controller.current = {
    data: { statuses: ["To Do"], rows: [] },
    banner: null,
    refresh: vi.fn(),
    sync: vi.fn(),
    onMove: vi.fn(),
    onOverride: vi.fn(),
    ...partial,
  };
}

describe("App view", () => {
  it("shows the loading state until the board resolves", () => {
    setController({ data: null });
    render(<App />);
    expect(screen.getByText("Loading…")).toBeInTheDocument();
  });

  it("renders the blocked-move banner and wires Override to onOverride", () => {
    const onOverride = vi.fn();
    setController({
      banner: { key: "PROJ-31", text: "Blocked moving PROJ-31 → Done: PR is not approved yet." },
      onOverride,
    });
    render(<App />);

    expect(screen.getByTestId("banner")).toHaveTextContent("Blocked moving PROJ-31");
    fireEvent.click(screen.getByTestId("override"));
    expect(onOverride).toHaveBeenCalledTimes(1);
  });

  it("wires the Sync button to the controller's sync action", () => {
    const sync = vi.fn();
    setController({ sync });
    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Sync" }));
    expect(sync).toHaveBeenCalledTimes(1);
  });
});

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "@/App";

describe("App", () => {
  it("loads and renders the board from the mock API on mount", async () => {
    render(<App />);
    // Header + the columns/cards appear once getBoard resolves.
    expect(await screen.findByText(/command center/)).toBeInTheDocument();
    expect(await screen.findByTestId("col-To Do")).toBeInTheDocument();
    expect(await screen.findByTestId("card-PROJ-12")).toBeInTheDocument();
    // The diverged issue surfaces its badge.
    expect(await screen.findByTestId("card-PROJ-44")).toHaveTextContent("diverged");
  });

  it("re-fetches the board when Sync is clicked", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Sync" }));
    // The board is still present after sync + refresh.
    expect(await screen.findByTestId("card-PROJ-12")).toBeInTheDocument();
  });
});

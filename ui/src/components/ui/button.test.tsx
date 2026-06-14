import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/ui/button";

describe("Button", () => {
  it("renders its children and forwards clicks", () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Sync</Button>);
    const btn = screen.getByRole("button", { name: "Sync" });
    fireEvent.click(btn);
    expect(onClick).toHaveBeenCalledOnce();
  });

  it("merges a custom className", () => {
    render(<Button className="extra-class">Go</Button>);
    expect(screen.getByRole("button", { name: "Go" })).toHaveClass("extra-class");
  });
});

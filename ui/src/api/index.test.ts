import { describe, expect, it } from "vitest";
import { api, isTauri } from "@/api";
import { mockApi } from "@/api/mock";

describe("api seam", () => {
  it("falls back to the mock API outside the Tauri shell", () => {
    // jsdom has no window.__TAURI_INTERNALS__.
    expect(isTauri).toBe(false);
    expect(api).toBe(mockApi);
  });
});

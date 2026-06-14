---
name: ui-testing
description: Work with the ui/ Playwright suite and regenerate the README board screenshot. Use when adding/debugging frontend e2e tests, when a click after a drag mysteriously does nothing, or when the UI changed and docs/board.png is stale.
---

# UI testing (Playwright) & the README screenshot

The frontend uses Playwright against a `pnpm build && preview` server, driving the
`mockApi` board data (browser has no Tauri). Tests live in `ui/tests/`.

## Running

```sh
pnpm --dir ui test:e2e     # the board suite (excludes the @screenshot test)
pnpm --dir ui screenshot   # regenerate docs/board.png (the @screenshot test only)
```

`test:e2e` and `screenshot` are split by a `@screenshot` title tag (`--grep` /
`--grep-invert`) so normal runs don't churn the committed PNG.

## Gotcha: a click right after a drag is swallowed

dnd-kit installs a **one-shot, capture-phase click listener** after a drag to cancel the
browser's synthetic post-drag click. In a test, the *next* click (real `.click()` *or*
`dispatchEvent`) lands inside that window and is eaten — the handler never fires, with no
error. A real user pauses, so the app is fine; the test isn't.

Fix: retry the click until it lands.

```ts
await expect(async () => {
  await page.getByTestId("override").click();
  await expect(page.getByTestId("col-Done")).toContainText("PROJ-31", { timeout: 1000 });
}).toPass();
```

When a post-drag interaction "does nothing", suspect this before hunting for an app bug.
Diagnose by reading `test-results/<test>/error-context.md` — the page snapshot shows
whether the action actually took effect.

## Regenerating docs/board.png

`ui/tests/screenshot.spec.ts` renders the board at a framed viewport and writes
`../docs/board.png` (repo-root `docs/`, not gitignored — `ui/board.png` *is* ignored).
Run `pnpm --dir ui screenshot` after any board UI/theme change, then commit the PNG.
Theme lives in `ui/src/index.css` (`:root` = light default, `.dark` for the dark palette).

Capture new test know-how with the [self-improve] skill.

import { expect, test } from "@playwright/test";

// Regenerates docs/board.png, the board screenshot embedded in the README.
// Tagged @screenshot so it's excluded from the normal e2e run (which would
// otherwise churn the committed image); refresh it deliberately with
// `pnpm screenshot`. Uses the mock board data — realistic dummy data with
// managed/external sessions, git state, a worktree, and a divergence.
test.use({ viewport: { width: 1320, height: 400 }, deviceScaleFactor: 2 });

test("board @screenshot", async ({ page }) => {
  await page.goto("/");
  // Wait until the board is fully populated so the shot isn't captured mid-load.
  await expect(page.getByTestId("card-PROJ-12")).toContainText("managed running");
  await expect(page.getByTestId("card-PROJ-44")).toContainText("diverged");
  await page.screenshot({ path: "../docs/board.png" });
});

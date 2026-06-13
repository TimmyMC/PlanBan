import { expect, type Page, test } from "@playwright/test";

// dnd-kit needs a real pointer drag past its activation distance, in steps.
async function dragCardToColumn(page: Page, cardId: string, colId: string) {
  const card = page.getByTestId(cardId);
  const target = page.getByTestId(colId);
  await card.scrollIntoViewIfNeeded();
  const box = await card.boundingBox();
  const drop = await target.boundingBox();
  if (!box || !drop) throw new Error("missing layout boxes");

  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(drop.x + drop.width / 2, drop.y + 80, { steps: 12 });
  await page.mouse.up();
}

test("renders columns, cards, and the divergence badge", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByTestId("col-To Do")).toBeVisible();
  await expect(page.getByTestId("col-In Progress")).toBeVisible();
  await expect(page.getByTestId("col-In Review")).toBeVisible();
  await expect(page.getByTestId("col-Done")).toBeVisible();

  await expect(page.getByTestId("card-PROJ-12")).toBeVisible();
  // PROJ-44's tracker moved to Done out from under us.
  await expect(page.getByTestId("card-PROJ-44")).toContainText("diverged");

  await page.screenshot({ path: "board.png", fullPage: true });
});

test("surfaces live session, git, and worktree state on a card", async ({ page }) => {
  await page.goto("/");
  // PROJ-12 has a managed run, a dirty worktree 3 ahead — the overview's whole point.
  const proj12 = page.getByTestId("card-PROJ-12");
  await expect(proj12).toContainText("managed running");
  await expect(proj12).toContainText("dirty");
  await expect(proj12).toContainText("+3 ahead");
  await expect(proj12).toContainText("wt/PROJ-12");
  // PROJ-31 is an external (attached) session.
  await expect(page.getByTestId("card-PROJ-31")).toContainText("external idle");
});

test("dragging a card to another column moves it", async ({ page }) => {
  await page.goto("/");
  await dragCardToColumn(page, "card-PROJ-58", "col-In Progress"); // starts in "To Do"
  await expect(page.getByTestId("col-In Progress")).toContainText("PROJ-58");
});

test("a gated move shows the override banner", async ({ page }) => {
  await page.goto("/");
  await dragCardToColumn(page, "card-PROJ-31", "col-Done"); // In Review -> Done is gated
  await expect(page.getByTestId("banner")).toContainText("Blocked moving PROJ-31");
});

test("override resumes a blocked move and clears the banner", async ({ page }) => {
  await page.goto("/");
  await dragCardToColumn(page, "card-PROJ-31", "col-Done");
  await expect(page.getByTestId("banner")).toBeVisible();

  // dnd-kit installs a one-shot capture-phase listener that swallows the first
  // click after a drag, so retry until the override actually lands.
  await expect(async () => {
    await page.getByTestId("override").click();
    await expect(page.getByTestId("col-Done")).toContainText("PROJ-31", { timeout: 1000 });
  }).toPass();

  await expect(page.getByTestId("banner")).toHaveCount(0);
});

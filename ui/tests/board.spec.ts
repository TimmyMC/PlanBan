import { test, expect } from "@playwright/test";

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

test("dragging a card to another column moves it", async ({ page }) => {
  await page.goto("/");
  const card = page.getByTestId("card-PROJ-58"); // starts in "To Do"
  const target = page.getByTestId("col-In Progress");

  await card.scrollIntoViewIfNeeded();
  const box = await card.boundingBox();
  const drop = await target.boundingBox();
  if (!box || !drop) throw new Error("missing layout boxes");

  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  // dnd-kit needs movement past the activation distance, in steps.
  await page.mouse.move(drop.x + drop.width / 2, drop.y + 80, { steps: 12 });
  await page.mouse.up();

  await expect(page.getByTestId("col-In Progress")).toContainText("PROJ-58");
});

test("a gated move shows the override banner", async ({ page }) => {
  await page.goto("/");
  const card = page.getByTestId("card-PROJ-31"); // In Review
  const done = page.getByTestId("col-Done");

  const box = await card.boundingBox();
  const drop = await done.boundingBox();
  if (!box || !drop) throw new Error("missing layout boxes");

  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(drop.x + drop.width / 2, drop.y + 80, { steps: 12 });
  await page.mouse.up();

  await expect(page.getByTestId("banner")).toContainText("Blocked moving PROJ-31");
});

import { expect, test } from "@playwright/test";
import { installApiMocks } from "./moadimApiMocks";

test("machine label text selects without crashing the renderer or closing the popup", async ({ page }) => {
  await installApiMocks(page);
  await page.goto("/routines");
  const summary = page.getByLabel("Machine filter", { exact: true });
  await summary.click();
  // Click the label text, not the input: focus briefly leaves the disclosure
  // before the label's default action focuses its checkbox.
  const label = page.locator(".machine-filter-options label").filter({ hasText: "mini-lab" });
  const box = await label.boundingBox();
  if (!box) throw new Error("Machine label is not visible");
  await label.click({ position: { x: box.width - 8, y: box.height / 2 } });
  await expect(label.getByRole("checkbox")).toBeChecked();
  await expect(page.locator("details.machine-filter")).toHaveAttribute("open", "");
  await expect(page.getByText("Showing 3 of 3")).toBeVisible();
  await expect(summary).toHaveText("2 selected");
});

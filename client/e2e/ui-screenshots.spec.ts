import { expect, test, type Page } from "@playwright/test";
import { installApiMocks, NOW_MS } from "./moadimApiMocks";

test.beforeEach(async ({ page }) => {
  await freezeBrowserClock(page);
  await installApiMocks(page);
});

test("overview dashboard screenshot stays reviewable", async ({ page }, testInfo) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  await saveScreenshot(page, testInfo.project.name, "overview");
});

test("routines operations screenshot stays reviewable", async ({ page }, testInfo) => {
  await page.goto("/routines");
  await expect(page.getByRole("heading", { name: "Routines" })).toBeVisible();
  await expect(page.getByText("Daily release digest")).toBeVisible();
  await saveScreenshot(page, testInfo.project.name, "routines");
});

test("reliability screenshot stays reviewable", async ({ page }, testInfo) => {
  await page.goto("/reliability");
  await expect(page.getByRole("heading", { name: "Reliability" })).toBeVisible();
  await expect(page.getByText("Skill learning loop")).toBeVisible();
  await saveScreenshot(page, testInfo.project.name, "reliability");
});

async function freezeBrowserClock(page: Page) {
  await page.addInitScript((fixedNow: number) => {
    const RealDate = Date;
    class FrozenDate extends RealDate {
      constructor(value?: string | number | Date) {
        if (value === undefined) super(fixedNow);
        else super(value);
      }
      static now() {
        return fixedNow;
      }
    }
    Object.setPrototypeOf(FrozenDate, RealDate);
    globalThis.Date = FrozenDate as DateConstructor;
  }, NOW_MS);
}

async function saveScreenshot(page: Page, project: string, name: string) {
  await page.screenshot({
    path: `../docs/screenshots/ui-e2e/${project}-${name}.png`,
    fullPage: true,
    animations: "disabled",
  });
}

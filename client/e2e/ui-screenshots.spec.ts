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

const darkPages = [
  { path: "/", heading: "Overview", name: "dark-overview" },
  { path: "/routines", heading: "Routines", name: "dark-routines" },
  { path: "/reliability", heading: "Reliability", name: "dark-reliability" },
];

for (const pageCase of darkPages) {
  test(`dark ${pageCase.heading} screenshot stays reviewable`, async ({ page }, testInfo) => {
    test.skip(testInfo.project.name !== "chromium-desktop", "dark baselines are desktop-only");
    await forceDarkTheme(page);
    await page.goto(pageCase.path);
    await expect(page.getByRole("heading", { name: pageCase.heading })).toBeVisible();
    await saveScreenshot(page, testInfo.project.name, pageCase.name);
  });
}

test("routines operations screenshot stays reviewable", async ({ page }, testInfo) => {
  await page.goto("/routines");
  await expect(page.getByRole("heading", { name: "Routines" })).toBeVisible();
  await expect(page.getByText("Daily release digest")).toBeVisible();

  const tableWrap = page.locator(".routine-table").locator("..");
  if (testInfo.project.name === "chromium-phone") {
    await expect
      .poll(async () => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1))
      .toBe(true);
    await expect
      .poll(async () => tableWrap.evaluate((el) => el.scrollWidth <= el.clientWidth + 1))
      .toBe(true);
  } else {
    await expect
      .poll(async () =>
        page.locator(".routine-title-cell").first().evaluate((el) => el.getBoundingClientRect().width),
      )
      .toBeGreaterThan(140);
  }

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

async function forceDarkTheme(page: Page) {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.addInitScript(() => {
    localStorage.setItem("moadim.client.theme", "dark");
  });
}

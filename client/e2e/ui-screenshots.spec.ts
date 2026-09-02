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
  await expect(page.getByRole("combobox", { name: "Auto-refresh interval" })).toHaveCount(0);

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

  if (testInfo.project.name === "chromium-desktop") {
    await page.getByRole("button", { name: "ACTIONS ▾" }).first().click();
    await expect(page.getByRole("menu")).toBeVisible();
    await saveScreenshot(page, testInfo.project.name, "routines-actions-menu");
  }
});

test("routine editor exposes independent multi-cron inputs", async ({ page }, testInfo) => {
  await page.goto("/routines");
  await page.getByRole("button", { name: "+ NEW ROUTINE" }).click();
  await expect(page.getByText("NEW ROUTINE")).toBeVisible();

  await page.getByRole("textbox", { name: "Cron expression 1" }).fill("@daily");
  await page.getByRole("button", { name: "+ ADD CRON EXPRESSION" }).click();
  await page.getByRole("textbox", { name: "Cron expression 2" }).fill("@hourly");
  await expect(page.getByText("At 12:00 AM")).toBeVisible();
  await expect(page.locator(".cron-preview", { hasText: "Every hour" })).toBeVisible();

  await saveScreenshot(page, testInfo.project.name, "routine-editor-multi-cron");
});

test("routine calendar day details screenshot stays reviewable", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "chromium-desktop", "calendar popover baseline is desktop-only");
  await page.goto("/routines");
  await expect(page.getByRole("heading", { name: "Routines" })).toBeVisible();
  await page.getByRole("button", { name: "CALENDAR" }).click();
  await page.locator(".cal-day").first().click({ position: { x: 12, y: 12 } });
  await expect(page.getByRole("dialog", { name: "Calendar day details" })).toBeVisible();
  await expect(page.getByRole("button", { name: /Run .* now/ }).first()).toBeVisible();
  await saveScreenshot(page, testInfo.project.name, "routines-calendar-day-details");
});

test("routines filesystem screenshot stays reviewable", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "chromium-desktop", "filesystem tree baseline is desktop-only");
  await page.goto("/routines");
  await expect(page.getByRole("heading", { name: "Routines" })).toBeVisible();
  await page.getByRole("button", { name: "FILES" }).click();
  await expect(page.getByRole("tree", { name: "Routine filesystem" })).toBeVisible();
  await expect(page.getByRole("treeitem", { name: /ops folder/i })).toBeVisible();
  await expect(page.getByRole("treeitem", { name: /ops\/discord\/daily-digest routine/i })).toBeVisible();
  await saveScreenshot(page, testInfo.project.name, "routines-filesystem");
});

test("routine folder management dialog screenshot stays reviewable", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "chromium-desktop", "folder management baseline is desktop-only");
  await page.goto("/routines");
  await expect(page.getByRole("heading", { name: "Routines" })).toBeVisible();
  await page.getByRole("button", { name: "FILES" }).click();
  await page.getByRole("button", { name: "Move folder" }).first().click();
  await expect(page.getByRole("dialog", { name: "FOLDER MANAGEMENT" })).toBeVisible();
  await expect(page.getByLabel("Current path ops/discord/daily-digest")).toBeVisible();
  await saveScreenshot(page, testInfo.project.name, "routines-folder-management");
});

test("settings refresh cadence screenshot stays reviewable", async ({ page }, testInfo) => {
  await page.goto("/settings");
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  await expect(page.getByText("Appearance", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Light" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Dark" })).toBeVisible();
  await expect(page.getByText("Data refresh")).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Auto-refresh interval" })).toBeVisible();
  await expect(page.getByText("System health")).toBeVisible();
  await expect(page.getByText("OS crontab sync is healthy")).toBeVisible();
  await expect(page.locator('button[title="Switch to dark mode"]')).toHaveCount(0);
  await expect(page.locator('button[title="Switch to light mode"]')).toHaveCount(0);
  if (testInfo.project.name === "chromium-phone") {
    await expect
      .poll(async () => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1))
      .toBe(true);
  }
  await saveScreenshot(page, testInfo.project.name, "settings");
});

test("crontab sync failure warning screenshot stays reviewable", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "chromium-desktop", "warning baseline is desktop-only");
  await page.unroute("**/api/v1/**");
  await installApiMocks(page, { crontabSyncOk: false });
  await page.goto("/");
  await expect(page.getByText("⚠ CRON STALE")).toBeVisible();
  await saveScreenshot(page, testInfo.project.name, "crontab-sync-warning");
});

test("settings crontab recovery guidance screenshot stays reviewable", async ({ page }, testInfo) => {
  test.skip(testInfo.project.name !== "chromium-desktop", "recovery baseline is desktop-only");
  await page.unroute("**/api/v1/**");
  await installApiMocks(page, { crontabSyncOk: false });
  await page.goto("/settings");
  await expect(page.getByText("⚠ OS crontab sync needs attention")).toBeVisible();
  await expect(page.getByText(/Full Disk Access/)).toBeVisible();
  await saveScreenshot(page, testInfo.project.name, "settings-crontab-recovery");
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
      constructor(...args: ConstructorParameters<DateConstructor>) {
        if (args.length === 0) super(fixedNow);
        else if (args.length === 1) super(args[0]);
        else super(args[0], args[1], args[2] ?? 1, args[3] ?? 0, args[4] ?? 0, args[5] ?? 0, args[6] ?? 0);
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

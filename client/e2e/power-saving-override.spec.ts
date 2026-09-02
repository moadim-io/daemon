import { expect, test } from "@playwright/test";
import { installApiMocks } from "./moadimApiMocks";

test("a confirmed UI run overrides system power saving for one manual trigger", async ({ page }) => {
  await installApiMocks(page);

  let triggerCount = 0;
  await page.route("**/api/v1/routines/routine-daily-digest/trigger", async (route) => {
    triggerCount += 1;
    if (triggerCount === 1) {
      await route.fulfill({
        status: 423,
        contentType: "application/json",
        body: JSON.stringify({ error: "locked: system power saving is active" }),
      });
      return;
    }
    expect(JSON.parse(route.request().postData() ?? "{}")).toEqual({ override_system_power_saving: true });
    await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify({ id: "routine-daily-digest" }) });
  });

  let dialogMessage: string | undefined;
  page.once("dialog", async (dialog) => {
    dialogMessage = dialog.message();
    await dialog.accept();
  });
  await page.goto("/routines");
  await page.getByRole("button", { name: "Run now" }).first().click();
  await expect.poll(() => dialogMessage).toBe("System power saving is active. Run this routine anyway?");
  await expect.poll(() => triggerCount).toBe(2);
});

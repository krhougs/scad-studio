// Phase 7 @config-settings smoke. Loads /settings, writes an openscad path,
// saves, reloads the page, and verifies the saved value came back via
// ConfigLoad.

import { expect, test } from "@playwright/test";
import { clearServiceWorkerState, createHarness } from "./_smoke-harness";

const HARNESS = createHarness({ bindPort: 39185, vitePort: 5180 });
const SENTINEL = "/tmp/smoke-openscad-" + Date.now();

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
});

test("@config-settings save then reload preserves openscad_path", async ({
  page,
}) => {
  await page.goto(
    `${HARNESS.baseUrl}/settings?ws=${encodeURIComponent(HARNESS.wsUrl)}`,
  );
  await expect(page.getByTestId("settings-openscad-path")).toBeVisible({
    timeout: 30_000,
  });
  await page.getByTestId("settings-openscad-path").fill(SENTINEL);
  await page.getByTestId("settings-save").click();
  await expect(page.getByTestId("settings-status")).toHaveText("saved", {
    timeout: 15_000,
  });

  // reload the route; ConfigLoad fires again and must echo the sentinel.
  await page.goto(
    `${HARNESS.baseUrl}/settings?ws=${encodeURIComponent(HARNESS.wsUrl)}`,
  );
  await expect(page.getByTestId("settings-openscad-path")).toHaveValue(
    SENTINEL,
    { timeout: 30_000 },
  );
});

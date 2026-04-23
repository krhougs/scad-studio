// Phase 7 @export-slicer smoke. Verifies that the slicer panel renders and
// that ExportRun fires and reports a terminal status for a mesh tab.

import { expect, test } from "@playwright/test";
import { clearServiceWorkerState, createHarness } from "./_smoke-harness";

const HARNESS = createHarness({ bindPort: 39184, vitePort: 5179 });

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
});

test("@export-slicer slicer panel renders list or empty marker", async ({
  page,
}) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}`);
  await page
    .getByTestId("entry-model.stl")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-model.stl").click();
  await expect(page.getByTestId("slicer-panel")).toBeVisible();
  await expect(page.getByTestId("slicer-list")).toBeVisible({ timeout: 15_000 });
});

test("@export-slicer export reports a terminal status", async ({ page }) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}`);
  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-params-cube.scad")
    .waitFor({ state: "visible", timeout: 15_000 });
  await page.getByTestId("entry-params-cube.scad").click();
  // scad tab does not surface the export panel (mesh-only); open model.stl
  // back — it is a mesh tab and thus exposes the export UI.
  await page.getByTestId("entry-model.stl").click();
  await expect(page.getByTestId("export-panel")).toBeVisible();
  await page.getByTestId("export-run").click();
  await expect(page.getByTestId("export-status")).toContainText(
    /export done|export error/,
    { timeout: 30_000 },
  );
});

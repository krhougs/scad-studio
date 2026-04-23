// Phase 7 @parameters-presets smoke. Opens examples/params-cube.scad, adds /
// applies / removes a parameter override, and exercises preset save + load +
// delete against the sibling .presets.json file.

import { expect, test } from "@playwright/test";
import { rm } from "node:fs/promises";
import path from "node:path";
import {
  HOST_WORKSPACE,
  clearServiceWorkerState,
  createHarness,
} from "./_smoke-harness";

const HARNESS = createHarness({ bindPort: 39183, vitePort: 5178 });
const PRESET_FILE = path.join(
  HOST_WORKSPACE,
  "examples",
  "params-cube.scad.presets.json",
);

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
  // best-effort: remove any preset state the test left behind. The fixture
  // (see tests/studio-web-smoke-workspace/examples/params-cube.presets.json)
  // ships an empty {version,presets} file; the test may mutate it.
  try {
    await rm(PRESET_FILE, { force: true });
  } catch {
    // ignore
  }
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
});

async function openParamsCube(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}`);
  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-params-cube.scad")
    .waitFor({ state: "visible", timeout: 15_000 });
  await page.getByTestId("entry-params-cube.scad").click();
  await expect(page.getByTestId("parameters-panel")).toBeVisible();
}

test("@parameters-presets override and apply triggers preview", async ({
  page,
}) => {
  await openParamsCube(page);
  const draftName = page.getByTestId("parameter-draft-name");
  const draftValue = page.getByTestId("parameter-draft-value");
  await draftName.scrollIntoViewIfNeeded();
  await draftName.fill("size");
  await draftValue.fill("20");
  await page.getByTestId("parameter-add").click();
  await expect(page.getByTestId("parameter-row-size")).toBeVisible({
    timeout: 10_000,
  });
  await page.getByTestId("parameters-apply").click();
  await expect(page.getByTestId("scad-preview-status")).toContainText(
    /preview pending|preview ready|preview error/,
    { timeout: 30_000 },
  );
});

test("@parameters-presets save, load, delete round-trip", async ({ page }) => {
  await openParamsCube(page);

  // prepare an override so save has something to persist
  const draftName = page.getByTestId("parameter-draft-name");
  await draftName.scrollIntoViewIfNeeded();
  await draftName.fill("wall");
  await page.getByTestId("parameter-draft-value").fill("4");
  await page.getByTestId("parameter-add").click();
  await expect(page.getByTestId("parameter-row-wall")).toBeVisible({
    timeout: 10_000,
  });

  // save preset
  const presetName = page.getByTestId("preset-save-name");
  await presetName.scrollIntoViewIfNeeded();
  await presetName.fill("thick");
  await page.getByTestId("preset-save").click();
  await expect(page.getByTestId("preset-row-thick")).toBeVisible({
    timeout: 15_000,
  });

  // remove override locally then load preset back
  await page.getByTestId("preset-load-thick").click();
  await expect(page.getByTestId("parameter-row-wall")).toBeVisible();

  // delete preset
  await page.getByTestId("preset-delete-thick").click();
  await expect(page.getByTestId("preset-row-thick")).toHaveCount(0);
});

// Phase 7 @canvas-interaction smoke. Boots the same websocket-host + Vite dev
// harness as browser_smoke on an isolated port pair, opens a mesh tab, drives
// the canvas toolbar, and verifies the status bar / preset data-testid shape.

import { expect, test } from "@playwright/test";
import { clearServiceWorkerState, createHarness } from "./_smoke-harness";

const HARNESS = createHarness({ bindPort: 39182, vitePort: 5177 });

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
});

test("@canvas-interaction toolbar preset switches status", async ({ page }) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}`);
  await page
    .getByTestId("entry-model.stl")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-model.stl").click();
  await expect(page.getByTestId("canvas-toolbar")).toBeVisible();
  await expect(page.getByTestId("canvas-statusbar")).toBeVisible();

  await page.getByTestId("canvas-preset-front").click();
  await expect(page.getByTestId("canvas-status-preset")).toHaveText("front");

  await page.getByTestId("canvas-preset-iso").click();
  await expect(page.getByTestId("canvas-status-preset")).toHaveText("iso");

  await page.getByTestId("canvas-zoom-in").click();
  await expect(page.getByTestId("canvas-status-preset")).toHaveText("custom");
});

test("@canvas-interaction pointer drag updates position", async ({ page }) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}`);
  await page
    .getByTestId("entry-model.stl")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-model.stl").click();
  await expect(page.getByTestId("viewer-pointer-stage")).toBeVisible();

  const before = (
    await page.getByTestId("canvas-status-position").textContent()
  )?.trim();

  const stage = page.getByTestId("viewer-pointer-stage");
  const box = await stage.boundingBox();
  if (!box) throw new Error("pointer stage has no bounding box");
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.move(cx, cy);
  await page.mouse.down({ button: "left" });
  await page.mouse.move(cx + 60, cy + 20, { steps: 5 });
  await page.mouse.up({ button: "left" });

  await expect
    .poll(async () => {
      const txt = await page.getByTestId("canvas-status-position").textContent();
      return txt?.trim();
    })
    .not.toBe(before);
});

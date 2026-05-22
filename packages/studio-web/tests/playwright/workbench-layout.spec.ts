import { cpSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { expect, test } from "@playwright/test";
import {
  clearServiceWorkerState,
  createHarness,
  HOST_WORKSPACE,
} from "./_smoke-harness";

const TEST_WORKSPACE = mkdtempSync(
  path.join(tmpdir(), "scad-studio-layout-workspace-"),
);
cpSync(HOST_WORKSPACE, TEST_WORKSPACE, { recursive: true });
const HARNESS = createHarness({
  bindPort: 39221,
  vitePort: 5221,
  workspacePath: TEST_WORKSPACE,
});

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
  rmSync(TEST_WORKSPACE, { recursive: true, force: true });
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
});

test("@workbench-layout follows the large desktop column design", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1920, height: 1080 });
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );

  await expect(page.getByTestId("workbench-left-panel")).toBeVisible({
    timeout: 30_000,
  });
  const rail = await page.locator(".rail").boundingBox();
  const leftPanel = await page.getByTestId("workbench-left-panel").boundingBox();
  const canvas = await page.getByTestId("workbench-canvas").boundingBox();
  const inspector = await page.getByTestId("workbench-inspector").boundingBox();
  if (!rail || !leftPanel || !canvas || !inspector) {
    throw new Error("workbench columns must all have layout boxes");
  }

  expect(canvas.width).toBeGreaterThan(leftPanel.width);
  expect(canvas.width).toBeGreaterThan(inspector.width);
  expect(leftPanel.x).toBeGreaterThanOrEqual(rail.x + rail.width - 1);
  expect(canvas.x).toBeGreaterThanOrEqual(leftPanel.x + leftPanel.width - 1);
  expect(inspector.x).toBeGreaterThanOrEqual(canvas.x + canvas.width - 1);
});

test("@workbench-layout files live in the left panel with explicit type labels", async ({
  page,
}) => {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );

  await expect(page.getByTestId("workbench-left-panel")).toBeVisible({
    timeout: 30_000,
  });
  await expect(page.getByTestId("left-panel-files")).toBeVisible();
  await expect(page.getByTestId("entry-kind-model.stl")).toHaveText("STL");
  await expect(page.getByTestId("entry-kind-README.md")).toHaveText("MD");
  await expect(page.getByTestId("inspector-entries")).toHaveCount(0);
});

test("@workbench-layout settings and log are left panel tabs controlled by route search params", async ({
  page,
}) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}`);

  await page.getByTestId("rail-settings").click();
  await expect(page).toHaveURL(/&left-panel=settings$/);
  await expect(page.getByTestId("left-panel-settings")).toBeVisible({
    timeout: 30_000,
  });

  await page.getByTestId("rail-log").click();
  await expect(page).toHaveURL(/&left-panel=log$/);
  await expect(page.getByTestId("left-panel-log")).toBeVisible();

  const logButton = await page.getByTestId("rail-log").boundingBox();
  const settingsButton = await page.getByTestId("rail-settings").boundingBox();
  if (!logButton || !settingsButton) {
    throw new Error("rail footer buttons must have layout boxes");
  }
  expect(logButton.y).toBeLessThan(settingsButton.y);
  await expect(page.getByText(/§ log/i)).toBeVisible();
  await expect(page.locator('[data-testid="left-panel-log"] .side-panel__head .sub')).toContainText("entries");
});

test("@workbench-layout restores left panel tab from search params and browser history", async ({
  page,
}) => {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=log`,
  );
  await expect(page.getByTestId("left-panel-log")).toBeVisible({
    timeout: 30_000,
  });

  await page.getByTestId("rail-files").click();
  await expect(page).toHaveURL(/&left-panel=files$/);
  await expect(page.getByTestId("left-panel-files")).toBeVisible();

  await page.goBack();
  await expect(page).toHaveURL(/&left-panel=log$/);
  await expect(page.getByTestId("left-panel-log")).toBeVisible();
});

test("@workbench-layout settings route redirects to side panel and preserves ws query", async ({
  page,
}) => {
  const encodedWs = encodeURIComponent(HARNESS.wsUrl);
  await page.goto(`${HARNESS.baseUrl}/settings?ws=${encodedWs}`);

  await expect(page).toHaveURL(new RegExp(`/\\?ws=${encodedWs}&left-panel=settings$`));
  await expect(page.getByTestId("left-panel-settings")).toBeVisible({
    timeout: 30_000,
  });
});

test("@workbench-layout scad parameters and presets render inside the right inspector", async ({
  page,
}) => {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );
  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-params-cube.scad")
    .waitFor({ state: "visible", timeout: 15_000 });
  await page.getByTestId("entry-params-cube.scad").click();

  await expect(
    page.getByTestId("workbench-inspector").getByTestId("parameters-panel"),
  ).toBeVisible({ timeout: 30_000 });
  await expect(
    page.getByTestId("workbench-inspector").getByTestId("presets-panel"),
  ).toBeVisible();
  await expect(
    page.getByTestId("workbench-canvas").getByTestId("parameters-panel"),
  ).toHaveCount(0);
  await expect(
    page.getByTestId("workbench-canvas").getByTestId("presets-panel"),
  ).toHaveCount(0);
});

test("@workbench-layout document title follows active file", async ({ page }) => {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );
  await expect(page).toHaveTitle("budn'");

  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-cube.scad")
    .waitFor({ state: "visible", timeout: 15_000 });
  await page.getByTestId("entry-cube.scad").click();
  await expect(page).toHaveTitle("cube.scad · budn'");

  await page.getByTestId("entry-README.md").click();
  await expect(page).toHaveTitle("README.md · budn'");
});

test("@workbench-layout right inspector sections collapse and expand", async ({
  page,
}) => {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );
  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-params-cube.scad")
    .waitFor({ state: "visible", timeout: 15_000 });
  await page.getByTestId("entry-params-cube.scad").click();

  for (const section of [
    "preview",
    "config",
    "parameters",
    "presets",
    "export",
    "slicers",
  ]) {
    await expect(page.getByTestId(`inspector-section-${section}`)).toBeVisible({
      timeout: 30_000,
    });
    await expect(page.getByTestId(`inspector-section-${section}-toggle`)).toContainText("-");
    await page.getByTestId(`inspector-section-${section}-toggle`).click();
    await expect(page.getByTestId(`inspector-section-${section}-body`)).toBeHidden();
    await expect(page.getByTestId(`inspector-section-${section}-toggle`)).toContainText("+");
    await page.getByTestId(`inspector-section-${section}-toggle`).click();
    await expect(page.getByTestId(`inspector-section-${section}-body`)).toBeVisible();
  }
});

test("@workbench-layout opening the same file twice reuses the existing tab", async ({
  page,
}) => {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );
  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-README.md")
    .waitFor({ state: "visible", timeout: 15_000 });

  await page.getByTestId("entry-README.md").click();
  await page.getByTestId("entry-cube.scad").click();
  await page.getByTestId("entry-README.md").click();

  await expect(page.getByTestId("tab-README.md")).toHaveCount(1);
  await expect(page.getByTestId("tab-README.md")).toHaveAttribute(
    "aria-selected",
    "true",
  );
});

test("@workbench-layout panel resize handles update column widths", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1920, height: 1080 });
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );
  await expect(page.getByTestId("workbench-left-panel")).toBeVisible({
    timeout: 30_000,
  });

  const before = await page.getByTestId("workbench-left-panel").boundingBox();
  const handle = await page.getByTestId("resize-left-panel").boundingBox();
  if (!before || !handle) throw new Error("left panel and handle must render");

  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle.x + 96, handle.y + handle.height / 2);
  await page.mouse.up();

  const after = await page.getByTestId("workbench-left-panel").boundingBox();
  if (!after) throw new Error("left panel must render after resize");
  expect(after.width).toBeGreaterThan(before.width + 40);
});

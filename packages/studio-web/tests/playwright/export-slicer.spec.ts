import { mkdtempSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import path from "node:path";
import { expect, test } from "@playwright/test";
import {
  clearRecordedClientCommands,
  clearServiceWorkerState,
  createHarness,
  installProtocolRecorder,
  latestRecordedClientCommand,
} from "./_smoke-harness";

const TMP_HOME = mkdtempSync(path.join(tmpdir(), "scad-studio-export-home-"));
const TMP_XDG = path.join(TMP_HOME, ".config");
const REAL_HOME = homedir();
const HARNESS = createHarness({
  bindPort: 39184,
  vitePort: 5179,
  hostEnv: {
    HOME: TMP_HOME,
    XDG_CONFIG_HOME: TMP_XDG,
    CARGO_HOME: process.env.CARGO_HOME ?? path.join(REAL_HOME, ".cargo"),
    RUSTUP_HOME: process.env.RUSTUP_HOME ?? path.join(REAL_HOME, ".rustup"),
  },
});
const SLICER_NAME = "smoke-slicer";
const TOOL_PATH = "/usr/bin/true";

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
  try {
    rmSync(TMP_HOME, { recursive: true, force: true });
  } catch {
    // best-effort cleanup
  }
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
  await installProtocolRecorder(page);
});

test("@export-slicer scad tab exposes slicer actions instead of a read-only list", async ({
  page,
}) => {
  await configureTools(page);
  await openScadWorkbench(page);
  const rightInspector = inspector(page);

  await expect(rightInspector.getByTestId("slicer-panel")).toBeVisible();
  await expect(rightInspector.getByTestId("slicer-list")).toBeVisible({
    timeout: 15_000,
  });
  await expect(rightInspector.getByTestId(`slicer-row-${SLICER_NAME}`)).toBeVisible();
  await expect(rightInspector.getByTestId(`slicer-send-${SLICER_NAME}`)).toBeVisible();

  await clearRecordedClientCommands(page);
  await rightInspector.getByTestId(`slicer-send-${SLICER_NAME}`).click();
  await expect
    .poll(
      async () => latestRecordedClientCommand(page, "export_run"),
      { timeout: 15_000 },
    )
    .toMatchObject({
      configured_openscad_path: TOOL_PATH,
      configured_slicers: [{ name: SLICER_NAME, path: TOOL_PATH }],
      slicer_name: SLICER_NAME,
    });
  await expect(rightInspector.getByTestId("slicer-status")).toContainText(
    new RegExp(`sent to ${SLICER_NAME}|export error`, "i"),
    { timeout: 30_000 },
  );
});

async function configureTools(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=settings`,
  );
  await expect(page.getByTestId("settings-openscad-path")).toBeVisible({
    timeout: 30_000,
  });
  await page.getByTestId("settings-openscad-path").fill(TOOL_PATH);
  await page.getByTestId("settings-slicer-name").fill(SLICER_NAME);
  await page.getByTestId("settings-slicer-path").fill(TOOL_PATH);
  await page.getByTestId("settings-slicer-add").click();
  await page.getByTestId("settings-save").click();
  await expect(page.getByTestId("settings-status")).toHaveText("saved", {
    timeout: 15_000,
  });
}

function inspector(page: import("@playwright/test").Page) {
  return page.getByTestId("workbench-inspector");
}

async function openScadWorkbench(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-params-cube.scad")
    .waitFor({ state: "visible", timeout: 15_000 });
  await page.getByTestId("entry-params-cube.scad").click();
  await expect(page.getByTestId("mesh-canvas")).toBeVisible({ timeout: 30_000 });
}

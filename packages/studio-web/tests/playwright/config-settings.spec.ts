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

const TMP_HOME = mkdtempSync(path.join(tmpdir(), "scad-studio-smoke-home-"));
const TMP_XDG = path.join(TMP_HOME, ".config");
const REAL_HOME = homedir();
const HARNESS = createHarness({
  bindPort: 39185,
  vitePort: 5180,
  hostEnv: {
    HOME: TMP_HOME,
    XDG_CONFIG_HOME: TMP_XDG,
    CARGO_HOME: process.env.CARGO_HOME ?? path.join(REAL_HOME, ".cargo"),
    RUSTUP_HOME: process.env.RUSTUP_HOME ?? path.join(REAL_HOME, ".rustup"),
  },
});
const OPENSCAD_PATH = "/usr/bin/true";
const SLICER_NAME = "smoke-slicer";
const SLICER_PATH = "/usr/bin/true";
const FLOATING_PANEL_OPACITY = "0.42";

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

test("@config-settings save then reload preserves editable config fields", async ({
  page,
}) => {
  await gotoSettingsPanel(page);
  await saveSettings(page);

  await gotoSettingsPanel(page);
  await expect(page.getByTestId("settings-openscad-path")).toHaveValue(
    OPENSCAD_PATH,
    { timeout: 30_000 },
  );
  await expect(page.getByTestId("settings-floating-panel-opacity")).toHaveValue(
    FLOATING_PANEL_OPACITY,
  );
  await expect(page.getByTestId("settings-display-unit")).toHaveValue("inch");
  await expect(page.getByTestId(`settings-slicer-row-${SLICER_NAME}`)).toBeVisible();
  await expect(
    page.getByTestId(`settings-slicer-path-${SLICER_NAME}`),
  ).toHaveValue(SLICER_PATH);
  await expect(page.getByTestId("settings-slicer-count")).toHaveText("1");
});

test("@config-settings saved config is consumed by preview, slicer list and export requests", async ({
  page,
}) => {
  await gotoSettingsPanel(page);
  await saveSettings(page);

  await openCubeScad(page);
  await expect(page.getByTestId("mesh-canvas")).toBeVisible({ timeout: 30_000 });

  await expect
    .poll(
      async () => latestRecordedClientCommand(page, "preview_request"),
      { timeout: 15_000 },
    )
    .toMatchObject({
      configured_openscad_path: OPENSCAD_PATH,
    });

  await expect
    .poll(
      async () => latestRecordedClientCommand(page, "slicer_list"),
      { timeout: 15_000 },
    )
    .toMatchObject({
      configured: [{ name: SLICER_NAME, path: SLICER_PATH }],
    });

  await clearRecordedClientCommands(page);
  await page.getByTestId("export-run").click();
  await expect
    .poll(
      async () => latestRecordedClientCommand(page, "export_run"),
      { timeout: 15_000 },
    )
    .toMatchObject({
      configured_openscad_path: OPENSCAD_PATH,
      configured_slicers: [{ name: SLICER_NAME, path: SLICER_PATH }],
      slicer_name: null,
    });

  await openModelStl(page);
  await expect(page.getByTestId("preview-mesh-size")).toContainText("in", {
    timeout: 30_000,
  });
});

async function gotoSettingsPanel(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=settings`,
  );
  await expect(page.getByTestId("left-panel-settings")).toBeVisible({
    timeout: 30_000,
  });
}

async function saveSettings(
  page: import("@playwright/test").Page,
): Promise<void> {
  await expect(page.getByTestId("settings-openscad-path")).toBeVisible({
    timeout: 30_000,
  });
  await page.getByTestId("settings-openscad-path").fill(OPENSCAD_PATH);
  await page.getByTestId("settings-floating-panel-opacity").fill(
    FLOATING_PANEL_OPACITY,
  );
  await page.getByTestId("settings-display-unit").selectOption("inch");
  await page.getByTestId("settings-slicer-name").fill(SLICER_NAME);
  await page.getByTestId("settings-slicer-path").fill(SLICER_PATH);
  await page.getByTestId("settings-slicer-add").click();
  await expect(page.getByTestId(`settings-slicer-row-${SLICER_NAME}`)).toBeVisible({
    timeout: 15_000,
  });
  await page.getByTestId("settings-save").click();
  await expect(page.getByTestId("settings-status")).toHaveText("saved", {
    timeout: 15_000,
  });
}

async function openCubeScad(
  page: import("@playwright/test").Page,
): Promise<void> {
  await clearRecordedClientCommands(page);
  await page.getByTestId("rail-files").click();
  await page.getByTestId("entry-examples").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-examples").click();
  await page.getByTestId("entry-cube.scad").waitFor({
    state: "visible",
    timeout: 15_000,
  });
  await page.getByTestId("entry-cube.scad").click();
}

async function openModelStl(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.getByTestId("rail-files").click();
  await page.getByTestId("entry-model.stl").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-model.stl").click();
}

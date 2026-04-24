// @parameters-presets smoke. Opens examples/params-cube.scad, adds / applies /
// removes a parameter override, and exercises preset save + load + delete
// against the desktop-compatible sibling .scad.json file.

import { expect, test } from "@playwright/test";
import { cpSync, mkdtempSync, rmSync } from "node:fs";
import { readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  HOST_WORKSPACE,
  clearRecordedClientCommands,
  clearServiceWorkerState,
  createHarness,
  installProtocolRecorder,
  latestRecordedClientCommand,
} from "./_smoke-harness";

const TEST_WORKSPACE = mkdtempSync(
  path.join(tmpdir(), "scad-studio-params-workspace-"),
);
cpSync(HOST_WORKSPACE, TEST_WORKSPACE, { recursive: true });
const HARNESS = createHarness({
  bindPort: 39183,
  vitePort: 5178,
  workspacePath: TEST_WORKSPACE,
});
const PRESET_FILE = path.join(
  TEST_WORKSPACE,
  "examples",
  "params-cube.scad.json",
);
const LEGACY_PRESET_FILE = path.join(
  TEST_WORKSPACE,
  "examples",
  "params-cube.presets.json",
);
const LEGACY_WEB_PRESET_FILE = path.join(
  TEST_WORKSPACE,
  "examples",
  "params-cube.scad.presets.json",
);

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
  try {
    await rm(PRESET_FILE, { force: true });
    await rm(LEGACY_PRESET_FILE, { force: true });
    await rm(LEGACY_WEB_PRESET_FILE, { force: true });
    rmSync(TEST_WORKSPACE, { recursive: true, force: true });
  } catch {
    // ignore
  }
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
  await installProtocolRecorder(page);
  await rm(PRESET_FILE, { force: true });
  await rm(LEGACY_PRESET_FILE, { force: true });
  await rm(LEGACY_WEB_PRESET_FILE, { force: true });
});

async function openParamsCube(
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
  await expect(inspector(page).getByTestId("parameters-panel")).toBeVisible();
  await expect(
    page.getByTestId("workbench-canvas").getByTestId("parameters-panel"),
  ).toHaveCount(0);
  await expect(
    page.getByTestId("workbench-canvas").getByTestId("presets-panel"),
  ).toHaveCount(0);
}

function inspector(page: import("@playwright/test").Page) {
  return page.getByTestId("workbench-inspector");
}

test("@parameters-presets typed controls drive current defines", async ({
  page,
}) => {
  await openParamsCube(page);

  await expect
    .poll(
      async () => latestRecordedClientCommand(page, "preview_request"),
      { timeout: 15_000 },
    )
    .toMatchObject({
      defines: ["size=10", "wall=2", "enabled=true", 'mode="draft"'],
    });

  const rightInspector = inspector(page);
  const size = rightInspector.getByTestId("parameter-control-size");
  await expect(size).toHaveAttribute("type", "number", { timeout: 10_000 });
  await expect(size).toHaveValue("10");
  await expect(rightInspector.getByTestId("parameter-control-enabled")).toBeChecked();
  await expect(rightInspector.getByTestId("parameter-control-mode")).toHaveValue("draft");

  await size.fill("24");
  await rightInspector.getByTestId("parameter-control-enabled").uncheck();
  await rightInspector.getByTestId("parameter-control-mode").selectOption("fine");
  await expect
    .poll(
      async () => latestRecordedClientCommand(page, "preview_request"),
      { timeout: 15_000 },
    )
    .toMatchObject({
      defines: ["size=24", "wall=2", "enabled=false", 'mode="fine"'],
    });

  await clearRecordedClientCommands(page);
  await rightInspector.getByTestId("parameter-restore-size").click();
  await expect(size).toHaveValue("10");
  await expect
    .poll(
      async () => latestRecordedClientCommand(page, "preview_request"),
      { timeout: 15_000 },
    )
    .toMatchObject({
      defines: ["size=10", "wall=2", "enabled=false", 'mode="fine"'],
    });
});

test("@parameters-presets save, load, delete round-trip", async ({ page }) => {
  await openParamsCube(page);
  const rightInspector = inspector(page);

  await rightInspector.getByTestId("parameter-control-wall").fill("4");

  // save preset
  const presetName = rightInspector.getByTestId("preset-save-name");
  await presetName.scrollIntoViewIfNeeded();
  await presetName.fill("thick");
  await rightInspector.getByTestId("preset-save").click();
  await expect(rightInspector.getByTestId("preset-row-thick")).toBeVisible({
    timeout: 15_000,
  });
  const persisted = JSON.parse(await readFile(PRESET_FILE, "utf-8")) as {
    presets?: Record<string, Record<string, unknown>>;
  };
  expect(persisted).toEqual({
    presets: {
      thick: {
        enabled: true,
        mode: "draft",
        size: 10,
        wall: 4,
      },
    },
  });

  await rightInspector.getByTestId("parameter-control-wall").fill("2");
  await rightInspector.getByTestId("preset-load-thick").click();
  await expect(rightInspector.getByTestId("parameter-control-wall")).toHaveValue("4");

  // delete preset
  await rightInspector.getByTestId("preset-delete-thick").click();
  await expect(rightInspector.getByTestId("preset-row-thick")).toHaveCount(0);
});

test("@parameters-presets desktop-compatible .scad.json is recognized", async ({
  page,
}) => {
  await writeFile(
    PRESET_FILE,
    JSON.stringify(
      {
        presets: {
          desktop: {
            enabled: true,
            size: 24,
            wall: 3,
          },
        },
      },
      null,
      2,
    ),
  );
  await openParamsCube(page);
  const rightInspector = inspector(page);
  await expect(rightInspector.getByTestId("preset-row-desktop")).toBeVisible({
    timeout: 15_000,
  });
  await rightInspector.getByTestId("preset-load-desktop").click();
  await expect(rightInspector.getByTestId("parameter-control-size")).toHaveValue("24");
  await expect(rightInspector.getByTestId("parameter-control-wall")).toHaveValue("3");
  await expect(rightInspector.getByTestId("parameter-control-enabled")).toBeChecked();
});

test("@parameters-presets legacy .presets.json path is still readable", async ({
  page,
}) => {
  await rm(PRESET_FILE, { force: true });
  await writeFile(
    LEGACY_PRESET_FILE,
    JSON.stringify({
      version: 1,
      presets: [{ name: "legacy", defines: ["wall=6"] }],
    }),
  );
  await openParamsCube(page);
  const rightInspector = inspector(page);
  await expect(rightInspector.getByTestId("preset-row-legacy")).toBeVisible({
    timeout: 15_000,
  });
  await rightInspector.getByTestId("preset-load-legacy").click();
  await expect(rightInspector.getByTestId("parameter-control-wall")).toHaveValue("6");
});

test("@parameters-presets old web .scad.presets.json path is still readable", async ({
  page,
}) => {
  await rm(PRESET_FILE, { force: true });
  await rm(LEGACY_PRESET_FILE, { force: true });
  await writeFile(
    LEGACY_WEB_PRESET_FILE,
    JSON.stringify({
      version: 1,
      presets: [{ name: "old-web", defines: ["wall=7"] }],
    }),
  );
  await openParamsCube(page);
  const rightInspector = inspector(page);
  await expect(rightInspector.getByTestId("preset-row-old-web")).toBeVisible({
    timeout: 15_000,
  });
  await rightInspector.getByTestId("preset-load-old-web").click();
  await expect(rightInspector.getByTestId("parameter-control-wall")).toHaveValue("7");
});

test("@parameters-presets switching scad tabs clears previous applied defines", async ({
  page,
}) => {
  await openParamsCube(page);
  const rightInspector = inspector(page);
  await rightInspector.getByTestId("parameter-control-wall").fill("5");
  await rightInspector.getByTestId("parameters-apply").click();

  await clearRecordedClientCommands(page);
  await page.getByTestId("entry-cube.scad").click();
  await expect(page.getByTestId("mesh-canvas")).toBeVisible({
    timeout: 30_000,
  });

  await expect
    .poll(
      async () => latestRecordedClientCommand(page, "preview_request"),
      { timeout: 15_000 },
    )
    .toMatchObject({
      defines: [],
    });

  await clearRecordedClientCommands(page);
  await rightInspector.getByTestId("export-run").click();
  await expect
    .poll(
      async () => latestRecordedClientCommand(page, "export_run"),
      { timeout: 15_000 },
    )
    .toMatchObject({
      defines: [],
    });
});

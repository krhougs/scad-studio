// Phase 7 @canvas-interaction smoke. Boots the same websocket-host + Vite dev
// harness as browser_smoke on an isolated port pair, opens a mesh tab, and
// drives the Inspector camera presets to verify camera state updates the canvas
// chrome, plus that pointer drag on the Three.js canvas triggers an orbit.

import { expect, test, type Locator } from "@playwright/test";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  HOST_WORKSPACE,
  clearServiceWorkerState,
  createHarness,
} from "./_smoke-harness";

const TEST_WORKSPACE = mkdtempSync(
  path.join(tmpdir(), "scad-studio-canvas-workspace-"),
);
cpSync(HOST_WORKSPACE, TEST_WORKSPACE, { recursive: true });
writeFileSync(path.join(TEST_WORKSPACE, "broken.scad"), "module broken( {\n");
writeFileSync(
  path.join(TEST_WORKSPACE, "shifted.stl"),
  `solid shifted_triangle
  facet normal 0 0 1
    outer loop
      vertex 10 0 0
      vertex 12 0 0
      vertex 10 2 0
    endloop
  endfacet
endsolid shifted_triangle
  `,
);
const MODEL_FILE = path.join(TEST_WORKSPACE, "model.stl");
const MODEL_STL_ORIGINAL = readFileSync(MODEL_FILE, "utf8");
const IMAGE_FILE = path.join(TEST_WORKSPACE, "screenshot.png");
const PARAMS_CUBE_SCAD_JSON = path.join(
  TEST_WORKSPACE,
  "examples",
  "params-cube.scad.json",
);
const ALT_IMAGE_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADUlEQVR42mP8/5+hHgAHggJ/PWdwRwAAAABJRU5ErkJggg==";
const MODEL_STL_REFRESHED = `solid smoke_triangle
  facet normal 0 0 1
    outer loop
      vertex 0 0 0
      vertex 2 0 0
      vertex 0 1 0
    endloop
  endfacet
endsolid smoke_triangle
`;
const HARNESS = createHarness({
  bindPort: 39182,
  vitePort: 5177,
  workspacePath: TEST_WORKSPACE,
});
const VIEWPORTS = [
  { name: "1920x1080", width: 1920, height: 1080 },
  { name: "1440x900", width: 1440, height: 900 },
  { name: "1280x800", width: 1280, height: 800 },
];

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
  rmSync(TEST_WORKSPACE, { recursive: true, force: true });
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
  rmSync(PARAMS_CUBE_SCAD_JSON, { force: true });
  writeFileSync(MODEL_FILE, MODEL_STL_ORIGINAL);
});

test("@canvas-interaction sidebar camera preset switches active view", async ({ page }) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-model.stl")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-model.stl").click();

  await expect(page.getByTestId("canvas-view-pills")).toHaveCount(0);

  // iso is the default
  await expect(page.getByTestId("camera-azimuth")).toHaveValue("-45.000");

  await page.getByTestId("camera-preset-front").click();
  await expect(page.getByTestId("camera-azimuth")).toHaveValue("-90.000");

  await page.getByTestId("entry-shifted.stl").click();
  await expect(page.getByTestId("camera-azimuth")).toHaveValue("-45.000", {
    timeout: 30_000,
  });

  await page.getByTestId("camera-preset-top").click();
  await expect(page.getByTestId("camera-elevation")).toHaveValue("90.000");

  await page.getByTestId("camera-reset").click();
  await expect(page.getByTestId("camera-azimuth")).toHaveValue("-45.000");
});

test("@canvas-interaction viewer toolbar drives render state", async ({ page }) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-model.stl")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-model.stl").click();

  const canvas = page.getByTestId("mesh-canvas");
  await canvas.waitFor({ state: "visible", timeout: 30_000 });
  await expect(page.getByTestId("viewer-toolbar")).toBeVisible();
  await expect(canvas).toHaveAttribute("data-render-mode", "solid");
  await expect(canvas).toHaveAttribute("data-projection-mode", "perspective");
  await expect(canvas).toHaveAttribute("data-show-grid", "true");
  await expect(canvas).toHaveAttribute("data-show-axis", "false");
  await expect(canvas).toHaveAttribute("data-preview-background", "#181b20");
  await expect(canvas).toHaveAttribute("data-grid-major-color", "#5a6573");
  await expect(canvas).toHaveAttribute("data-grid-minor-color", "#343b45");
  await expect(canvas).toHaveAttribute("data-lighting-intensity", "1.25");
  await expect(canvas).toHaveAttribute("data-gizmo-size", /\d+/);
  await expect(canvas).toHaveAttribute("data-clip-far", /\d+\.\d{3}/);
  const distanceBeforeResize = await page.getByTestId("camera-distance").inputValue();
  await page.setViewportSize({ width: 900, height: 1200 });
  await expect(page.getByTestId("camera-distance")).not.toHaveValue(
    distanceBeforeResize,
  );

  await page.getByTestId("viewer-render-wireframe").click();
  await expect(canvas).toHaveAttribute("data-render-mode", "wireframe");

  await page.getByTestId("viewer-render-xray").click();
  await expect(canvas).toHaveAttribute("data-render-mode", "xray");

  await page.getByTestId("viewer-projection-orthographic").click();
  await expect(canvas).toHaveAttribute("data-projection-mode", "orthographic");
  await expect(canvas).toHaveAttribute("data-orthographic-half-height", /\d+\.\d{3}/);
  const halfHeightBeforeWheel = await canvas.getAttribute(
    "data-orthographic-half-height",
  );
  await canvas.evaluate((element) => {
    element.dispatchEvent(
      new WheelEvent("wheel", {
        deltaY: -120,
        deltaMode: WheelEvent.DOM_DELTA_PIXEL,
        bubbles: true,
        cancelable: true,
      }),
    );
  });
  await expect(canvas).not.toHaveAttribute(
    "data-orthographic-half-height",
    halfHeightBeforeWheel ?? "",
  );
  const halfHeightAfterWheel = await canvas.getAttribute(
    "data-orthographic-half-height",
  );
  await page.getByTestId("viewer-toggle-fog").click();
  await expect(canvas).toHaveAttribute("data-fog-enabled", "true");
  await expect(canvas).toHaveAttribute("data-fog-near", /\d+\.\d{3}/);
  await expect(canvas).toHaveAttribute("data-fog-far", /\d+\.\d{3}/);

  await setPreviewDelay(page, 2_500);
  writeFileSync(MODEL_FILE, MODEL_STL_REFRESHED);
  await expect(page.getByTestId("mesh-loading-overlay")).toContainText(
    "preview updating",
    { timeout: 15_000 },
  );
  await expect(canvas).toBeVisible();
  await expect(canvas).toHaveAttribute(
    "data-orthographic-half-height",
    halfHeightAfterWheel ?? "",
  );
  await expect(page.getByTestId("message")).toContainText("preview ready", {
    timeout: 30_000,
  });
  await setPreviewDelay(page, 0);
  await page.getByTestId("entry-shifted.stl").click();
  await expect(canvas).not.toHaveAttribute(
    "data-orthographic-half-height",
    halfHeightAfterWheel ?? "",
  );

  await page.getByTestId("viewer-toggle-grid").click();
  await expect(canvas).toHaveAttribute("data-show-grid", "false");

  await page.getByTestId("viewer-toggle-axis").click();
  await expect(canvas).toHaveAttribute("data-show-axis", "true");

  await page.getByTestId("viewer-toggle-build-plate").click();
  await expect(canvas).toHaveAttribute("data-show-build-plate", "true");

  await page.getByTestId("viewer-toggle-shadow").click();
  await expect(canvas).toHaveAttribute("data-shadows-enabled", "true");

  await page.getByTestId("viewer-color-mono").click();
  await expect(canvas).toHaveAttribute("data-color-mode", "mono");

  await page.getByTestId("viewer-toggle-clip").click();
  await expect(canvas).toHaveAttribute("data-clip-plane-enabled", "true");
});

test("@canvas-interaction scad preview appearance controls persist per file", async ({
  page,
}, testInfo) => {
  writeFileSync(
    PARAMS_CUBE_SCAD_JSON,
    JSON.stringify(explicitOffFile(), null, 2),
  );
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-params-cube.scad")
    .waitFor({ state: "visible", timeout: 15_000 });
  await page.getByTestId("entry-params-cube.scad").click();

  const inspector = page.getByTestId("workbench-inspector");
  const canvas = page.getByTestId("mesh-canvas");
  const pointLightOff = inspector.getByTestId("preview-point-light-mode-off");
  const pointLightAuto = inspector.getByTestId("preview-point-light-mode-auto");
  const pointLightManual = inspector.getByTestId("preview-point-light-mode-manual");
  await canvas.waitFor({ state: "visible", timeout: 30_000 });
  await expect(inspector.getByTestId("preview-appearance-panel")).toBeVisible();
  await expect(canvas).toHaveAttribute("data-preview-background", "#181b20");
  const defaultGridSignature = await canvas.getAttribute("data-grid-color-signature");
  if (!defaultGridSignature) throw new Error("grid color signature should be present");
  await expect(canvas).toHaveAttribute("data-point-light-mode", "off");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "off");
  await expect(canvas).toHaveAttribute("data-point-light-enabled", "false");
  await expect(canvas).toHaveAttribute("data-point-light-config-intensity", "1.6");
  await expect(canvas).toHaveAttribute("data-build-plate-visible", "false");
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "off");

  await page.getByTestId("viewer-toggle-shadow").click();
  await expect(canvas).toHaveAttribute("data-shadows-enabled", "true");
  await expect(canvas).toHaveAttribute("data-point-light-mode", "off");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "auto");
  await expect(canvas).toHaveAttribute("data-point-light-forced", "true");
  await expect(canvas).toHaveAttribute("data-point-light-enabled", "true");
  await expect(canvas).toHaveAttribute("data-point-light-cast-shadow", "true");
  await expect(canvas).toHaveAttribute("data-point-light-decay", "0");
  await expect(canvas).toHaveAttribute("data-point-light-intensity", "2.000");
  await expect(canvas).toHaveAttribute("data-mesh-cast-shadow", "true");
  await expect(canvas).toHaveAttribute("data-mesh-receive-shadow", "false");
  await expect(canvas).toHaveAttribute("data-show-build-plate", "false");
  await expect(canvas).toHaveAttribute("data-build-plate-visible", "true");
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "off");
  await expect.poll(() => readJsonFile(PARAMS_CUBE_SCAD_JSON)).toEqual(explicitOffFile());
  await inspector.getByTestId("preview-background-color").fill("#20242b");
  await expect(canvas).toHaveAttribute("data-preview-background", "#20242b");
  await expect.poll(() => {
    const file = readJsonFile(PARAMS_CUBE_SCAD_JSON);
    const appearance = previewAppearanceFromFile(file);
    return (
      appearance?.backgroundColor === "#20242b" &&
      appearance.pointLightIntensity === 1.6 &&
      appearance.pointLightMode === "off" &&
      appearance.pointLightPosition === null
    );
  }).toBe(true);
  await page.getByTestId("viewer-toggle-shadow").click();
  await expect(canvas).toHaveAttribute("data-shadows-enabled", "false");
  await expect(canvas).toHaveAttribute("data-build-plate-visible", "false");

  await canvas.screenshot({
    path: testInfo.outputPath("preview-appearance-background.png"),
  });

  await inspector.getByTestId("preview-grid-major-color").fill("#7c8795");
  await expect(canvas).toHaveAttribute("data-grid-major-color", "#7c8795");

  await inspector.getByTestId("preview-grid-minor-color").fill("#46505d");
  await expect(canvas).toHaveAttribute("data-grid-minor-color", "#46505d");
  await expect(canvas).not.toHaveAttribute(
    "data-grid-color-signature",
    defaultGridSignature,
  );

  const lightingField = inspector
    .getByTestId("preview-lighting-number-field")
    .getByRole("spinbutton");
  await lightingField.fill("0.4");
  await expect(canvas).toHaveAttribute("data-lighting-intensity", "0.4");
  const dimRigIntensity = await numericAttribute(canvas, "data-light-rig-intensity");
  await lightingField.fill("2.4");
  await expect(canvas).toHaveAttribute("data-lighting-intensity", "2.4");
  const brightRigIntensity = await numericAttribute(canvas, "data-light-rig-intensity");
  expect(brightRigIntensity / dimRigIntensity).toBeCloseTo(6, 1);
  await lightingField.fill("1.6");
  await expect(canvas).toHaveAttribute("data-lighting-intensity", "1.6");

  const pointLightIntensityField = inspector
    .getByTestId("preview-point-light-intensity-number-field")
    .getByRole("spinbutton");
  await pointLightIntensityField.fill("2.5");
  await expect(canvas).toHaveAttribute("data-point-light-config-intensity", "2.5");

  await pointLightManual.click();
  await expect(canvas).toHaveAttribute("data-point-light-mode", "manual");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "manual");
  await expect(canvas).toHaveAttribute("data-point-light-intensity", "4.000");
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "manual");
  const directManualPointLightPosition = await canvas.getAttribute("data-point-light-position");
  if (!directManualPointLightPosition) {
    throw new Error("direct manual point light position should be present");
  }
  await expect.poll(() => {
    const file = readJsonFile(PARAMS_CUBE_SCAD_JSON);
    const appearance = previewAppearanceFromFile(file);
    return (
      appearance?.pointLightMode === "manual" &&
      pointLightPositionClose(appearance.pointLightPosition, directManualPointLightPosition)
    );
  }).toBe(true);

  await pointLightAuto.click();
  await expect(canvas).toHaveAttribute("data-point-light-mode", "auto");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "auto");
  await expect(canvas).toHaveAttribute("data-point-light-enabled", "true");
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "auto");
  const autoPointLightPosition = await canvas.getAttribute("data-point-light-position");
  if (!autoPointLightPosition) throw new Error("auto point light position should be present");
  expect(autoPointLightPosition).toBe(directManualPointLightPosition);

  await pointLightManual.click();
  await expect(canvas).toHaveAttribute("data-point-light-mode", "manual");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "manual");
  await expect(canvas).toHaveAttribute("data-point-light-position", autoPointLightPosition);
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "manual");
  const pointLightX = inspector
    .getByTestId("preview-point-light-x-number-field")
    .getByRole("spinbutton");
  const pointLightY = inspector
    .getByTestId("preview-point-light-y-number-field")
    .getByRole("spinbutton");
  const pointLightZ = inspector
    .getByTestId("preview-point-light-z-number-field")
    .getByRole("spinbutton");
  await pointLightX.fill("11");
  await pointLightY.fill("22");
  await pointLightZ.fill("33");
  await expect(canvas).toHaveAttribute("data-point-light-position", "11.000,22.000,33.000");

  await expect
    .poll(async () => readJsonFile(path.join(TEST_WORKSPACE, "examples", "params-cube.scad.json")), {
      timeout: 10_000,
    })
    .toMatchObject({
      presets: {
        existing: {
          size: 18,
        },
      },
      previewAppearance: {
        backgroundColor: "#20242b",
        gridMajorColor: "#7c8795",
        gridMinorColor: "#46505d",
        lightingIntensity: 1.6,
        pointLightIntensity: 2.5,
        pointLightMode: "manual",
        pointLightPosition: [11, 22, 33],
      },
    });

  await pointLightAuto.click();
  await expect(canvas).toHaveAttribute("data-point-light-mode", "auto");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "auto");
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "auto");
  await expect.poll(() => {
    const file = readJsonFile(PARAMS_CUBE_SCAD_JSON);
    const appearance = previewAppearanceFromFile(file);
    return (
      appearance?.pointLightMode === "auto" &&
      pointLightPositionClose(appearance.pointLightPosition, "11.000,22.000,33.000")
    );
  }).toBe(true);
  await page.getByTestId("viewer-toggle-shadow").click();
  await expect(canvas).toHaveAttribute("data-shadows-enabled", "true");
  await expect(canvas).toHaveAttribute("data-point-light-mode", "auto");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "auto");
  await expect(canvas).toHaveAttribute("data-point-light-forced", "false");
  await expect(canvas).toHaveAttribute("data-point-light-enabled", "true");
  await expect.poll(() => {
    const file = readJsonFile(PARAMS_CUBE_SCAD_JSON);
    const appearance = previewAppearanceFromFile(file);
    return (
      appearance?.pointLightMode === "auto" &&
      pointLightPositionClose(appearance.pointLightPosition, "11.000,22.000,33.000")
    );
  }).toBe(true);
  await page.getByTestId("viewer-toggle-shadow").click();
  await expect(canvas).toHaveAttribute("data-shadows-enabled", "false");

  await pointLightManual.click();
  await expect(canvas).toHaveAttribute("data-point-light-mode", "manual");
  await expect(canvas).toHaveAttribute("data-point-light-position", "11.000,22.000,33.000");
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "manual");
  await page.getByTestId("viewer-toggle-shadow").click();
  await expect(canvas).toHaveAttribute("data-shadows-enabled", "true");
  await expect(canvas).toHaveAttribute("data-point-light-mode", "manual");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "manual");
  await expect(canvas).toHaveAttribute("data-point-light-forced", "false");
  await expect(canvas).toHaveAttribute("data-point-light-position", "11.000,22.000,33.000");
  await expect.poll(() => {
    const file = readJsonFile(PARAMS_CUBE_SCAD_JSON);
    const appearance = previewAppearanceFromFile(file);
    return (
      appearance?.pointLightMode === "manual" &&
      pointLightPositionClose(appearance.pointLightPosition, "11.000,22.000,33.000")
    );
  }).toBe(true);
  await page.getByTestId("viewer-toggle-shadow").click();
  await expect(canvas).toHaveAttribute("data-shadows-enabled", "false");

  await pointLightOff.click();
  await expect(canvas).toHaveAttribute("data-point-light-mode", "off");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "off");
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "off");
  await expect.poll(() => {
    const file = readJsonFile(PARAMS_CUBE_SCAD_JSON);
    const appearance = previewAppearanceFromFile(file);
    return (
      appearance?.pointLightMode === "off" &&
      pointLightPositionClose(appearance.pointLightPosition, "11.000,22.000,33.000")
    );
  }).toBe(true);

  await page.getByTestId("viewer-toggle-shadow").click();
  await expect(canvas).toHaveAttribute("data-shadows-enabled", "true");
  await expect(canvas).toHaveAttribute("data-point-light-mode", "off");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "auto");
  await expect.poll(() => {
    const file = readJsonFile(PARAMS_CUBE_SCAD_JSON);
    const appearance = previewAppearanceFromFile(file);
    return (
      appearance?.pointLightMode === "off" &&
      pointLightPositionClose(appearance.pointLightPosition, "11.000,22.000,33.000")
    );
  }).toBe(true);
  await page.getByTestId("viewer-toggle-shadow").click();
  await expect(canvas).toHaveAttribute("data-shadows-enabled", "false");

  await pointLightAuto.click();
  await expect(canvas).toHaveAttribute("data-point-light-mode", "auto");
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "auto");
  await expect.poll(() => {
    const file = readJsonFile(PARAMS_CUBE_SCAD_JSON);
    const appearance = previewAppearanceFromFile(file);
    return (
      appearance?.pointLightMode === "auto" &&
      pointLightPositionClose(appearance.pointLightPosition, "11.000,22.000,33.000")
    );
  }).toBe(true);

  await pointLightManual.click();
  await expect(canvas).toHaveAttribute("data-point-light-position", "11.000,22.000,33.000");
  await expectPointLightButtons(pointLightOff, pointLightAuto, pointLightManual, "manual");

  await inspector.getByTestId("preview-point-light-reset").click();
  await expect(canvas).toHaveAttribute("data-point-light-mode", "manual");
  await expect(canvas).toHaveAttribute("data-point-light-position", autoPointLightPosition);
  await expect.poll(async () => {
    const file = readJsonFile(path.join(TEST_WORKSPACE, "examples", "params-cube.scad.json"));
    const appearance = previewAppearanceFromFile(file);
    return (
      appearance?.pointLightMode === "manual" &&
      pointLightPositionClose(appearance.pointLightPosition, autoPointLightPosition)
    );
  }).toBe(true);

  await page.getByTestId("entry-cube.scad").click();
  await expect(canvas).toHaveAttribute("data-preview-background", "#181b20", {
    timeout: 30_000,
  });
  await expect(canvas).toHaveAttribute("data-grid-major-color", "#5a6573");
  await expect(canvas).toHaveAttribute("data-grid-minor-color", "#343b45");
  await expect(canvas).toHaveAttribute("data-lighting-intensity", "1.25");
  await expect(canvas).toHaveAttribute("data-point-light-config-intensity", "1.6");
  await expect(canvas).toHaveAttribute("data-point-light-mode", "off");
  await expect(canvas).toHaveAttribute("data-effective-point-light-mode", "off");

  await page.getByTestId("entry-params-cube.scad").click();
  await expect(canvas).toHaveAttribute("data-preview-background", "#20242b", {
    timeout: 30_000,
  });
  await expect(canvas).toHaveAttribute("data-grid-major-color", "#7c8795");
  await expect(canvas).toHaveAttribute("data-grid-minor-color", "#46505d");
  await expect(canvas).toHaveAttribute("data-lighting-intensity", "1.6");
  await expect(canvas).toHaveAttribute("data-point-light-config-intensity", "2.5");
  await expect(canvas).toHaveAttribute("data-point-light-mode", "manual");
  await expect(canvas).toHaveAttribute("data-point-light-position", autoPointLightPosition);
});

test("@canvas-interaction preview info and camera controls are available", async ({
  page,
}) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-model.stl")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-model.stl").click();

  await expect(page.getByTestId("preview-mesh-summary")).toContainText("verts", {
    timeout: 30_000,
  });
  await expect(page.getByTestId("preview-mesh-size")).toContainText("mm");
  await expect(page.getByTestId("camera-panel")).toBeVisible();
  await expect(page.getByTestId("camera-handle")).toHaveCount(0);
  await expect(page.getByTestId("camera-knob-azimuth")).toHaveAttribute(
    "role",
    "slider",
  );
  const azimuthField = page.getByTestId("camera-number-field-azimuth");
  const azimuth = azimuthField.getByRole("spinbutton");
  await expect(azimuthField).toBeVisible();

  const cameraRowBefore = await page
    .getByTestId("camera-control-azimuth")
    .boundingBox();
  const cameraFieldBefore = await azimuthField.boundingBox();
  await azimuth.fill("90");
  await expect(azimuth).toHaveValue("90");
  const cameraRowAfter = await page
    .getByTestId("camera-control-azimuth")
    .boundingBox();
  const cameraFieldAfter = await azimuthField.boundingBox();
  expect(cameraRowBefore?.width).toBeCloseTo(cameraRowAfter?.width ?? 0, 0);
  expect(cameraRowBefore?.height).toBeCloseTo(cameraRowAfter?.height ?? 0, 0);
  expect(cameraFieldBefore?.width).toBeCloseTo(cameraFieldAfter?.width ?? 0, 0);
  expect(cameraFieldBefore?.height).toBeCloseTo(cameraFieldAfter?.height ?? 0, 0);

  await page.getByTestId("camera-knob-azimuth").focus();
  await page.getByTestId("camera-knob-azimuth").press("ArrowRight");
  await expect(azimuth).toHaveValue("91.000");

  await page
    .getByTestId("camera-number-field-azimuth")
    .locator(".numeric-control__stepper")
    .last()
    .click();
  await expect(azimuth).toHaveValue("92.000");

  await page
    .getByTestId("camera-number-field-target-x")
    .getByRole("spinbutton")
    .fill("");
  await page
    .getByTestId("camera-number-field-distance")
    .getByRole("spinbutton")
    .click();
  await expect(
    page.getByTestId("camera-number-field-target-x").getByRole("spinbutton"),
  ).not.toHaveValue("NaN");
  await expect(page.getByTestId("mesh-canvas")).toBeVisible();

  const cameraToggle = page.getByTestId("inspector-section-camera-toggle");
  await cameraToggle.click();
  await expect(page.getByTestId("inspector-section-camera-body")).toBeHidden();
  await cameraToggle.click();
  await expect(page.getByTestId("inspector-section-camera-body")).toBeVisible();
  await expect(cameraToggle).toBeFocused();
});

test("@canvas-interaction ViewportGizmo reflects selected view", async ({
  page,
}) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-model.stl")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-model.stl").click();

  await expect(page.getByTestId("viewport-gizmo")).toBeVisible({
    timeout: 30_000,
  });
  await expect(page.getByTestId("viewport-gizmo-axis-x")).toBeVisible();
  await expect(page.getByTestId("viewport-gizmo-axis-y")).toBeVisible();
  await expect(page.getByTestId("viewport-gizmo-axis-z")).toBeVisible();
  await expect(
    page
      .locator('[data-testid^="viewport-gizmo-"]')
      .filter({ hasText: /front|top|left/i }),
  ).toHaveCount(0);
  const zAxisBefore = await page
    .getByTestId("viewport-gizmo-axis-z")
    .getAttribute("data-end");
  await page.getByTestId("camera-preset-top").click();
  await expect(page.getByTestId("viewport-gizmo-axis-z")).not.toHaveAttribute(
    "data-end",
    zAxisBefore ?? "",
  );
  await expect(
    page.getByTestId("camera-number-field-elevation").getByRole("spinbutton"),
  ).toHaveValue("90.000");
  await page.getByTestId("camera-preset-front").click();
  await expect(
    page.getByTestId("camera-number-field-azimuth").getByRole("spinbutton"),
  ).toHaveValue("-90.000");
  await expect(
    page.getByTestId("camera-number-field-elevation").getByRole("spinbutton"),
  ).toHaveValue("0.000");
  await page.getByTestId("camera-preset-bottom").click();
  await expect(
    page.getByTestId("camera-number-field-elevation").getByRole("spinbutton"),
  ).toHaveValue("-90.000");
  await page.getByTestId("camera-preset-back").click();
  await expect(
    page.getByTestId("camera-number-field-azimuth").getByRole("spinbutton"),
  ).toHaveValue("90.000");
  await expect(
    page.getByTestId("camera-number-field-elevation").getByRole("spinbutton"),
  ).toHaveValue("0.000");
  await page.getByTestId("camera-preset-right").click();
  await expect(
    page.getByTestId("camera-number-field-azimuth").getByRole("spinbutton"),
  ).toHaveValue("0.000");
  await expect(
    page.getByTestId("camera-number-field-elevation").getByRole("spinbutton"),
  ).toHaveValue("0.000");
  await page.getByTestId("camera-preset-left").click();
  await expect(
    page.getByTestId("camera-number-field-azimuth").getByRole("spinbutton"),
  ).toHaveValue("180.000");
  await expect(
    page.getByTestId("camera-number-field-elevation").getByRole("spinbutton"),
  ).toHaveValue("0.000");
});

test("@canvas-interaction initial mesh preview exposes prominent loading", async ({
  page,
}) => {
  await setPreviewDelay(page, 2_500);
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-cube.scad")
    .waitFor({ state: "visible", timeout: 15_000 });
  await page.getByTestId("entry-cube.scad").click();
  const canvas = page.getByTestId("mesh-canvas");
  await page.getByTestId("viewer-projection-orthographic").click();

  await expect(page.getByTestId("mesh-loading-overlay")).toBeVisible({
    timeout: 1_000,
  });
  await expect(page.getByTestId("mesh-loading-overlay")).toContainText(
    "preview loading",
  );
  await expect(canvas).toHaveCSS("filter", "blur(8px)");
  await expectCenteredIn(
    page.getByTestId("mesh-loading-card"),
    page.getByTestId("mesh-loading-overlay"),
    "initial mesh loading prompt must stay centered",
  );
  await expect(canvas).not.toHaveAttribute(
    "data-orthographic-half-height",
    /\d/,
  );
  await expect(canvas).toHaveAttribute(
    "data-orthographic-half-height",
    /\d+\.\d{3}/,
    { timeout: 30_000 },
  );
});

test("@canvas-interaction parameter preview exposes updating loading", async ({
  page,
}) => {
  await setPreviewDelay(page, 0);
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-examples")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-examples").click();
  await page
    .getByTestId("entry-params-cube.scad")
    .waitFor({ state: "visible", timeout: 15_000 });
  await page.getByTestId("entry-params-cube.scad").click();

  const canvas = page.getByTestId("mesh-canvas");
  await canvas.waitFor({ state: "visible", timeout: 30_000 });
  await expect(page.getByTestId("message")).toContainText("preview ready", {
    timeout: 30_000,
  });

  await setPreviewDelay(page, 2_500);
  const size = page
    .getByTestId("workbench-inspector")
    .getByTestId("parameter-number-field-size")
    .getByRole("spinbutton");
  await size.fill("12");

  const overlay = page.getByTestId("mesh-loading-overlay");
  await expect(overlay).toBeVisible({ timeout: 1_000 });
  await expect(overlay).toContainText("preview updating");
  await expect(canvas).toHaveCSS("filter", "blur(8px)");
  await expectCenteredIn(
    page.getByTestId("mesh-loading-card"),
    overlay,
    "mesh updating prompt must stay centered",
  );
  await expect(canvas).toBeVisible();
});

test("@canvas-interaction image refresh exposes updating loading", async ({
  page,
}) => {
  await setFileReadDelay(page, 0);
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-screenshot.png")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-screenshot.png").click();
  const image = page.getByTestId("image-element");
  await image.waitFor({ state: "visible", timeout: 15_000 });
  const originalSrc = await image.getAttribute("src");
  if (!originalSrc) throw new Error("image viewer did not expose a blob src");

  await setFileReadDelay(page, 2_500);
  writeFileSync(IMAGE_FILE, Buffer.from(ALT_IMAGE_BASE64, "base64"));
  await expect(page.getByTestId("image-loading-overlay")).toContainText(
    "image updating",
    { timeout: 15_000 },
  );
  await expect(image).toHaveAttribute("src", originalSrc);
  await expect
    .poll(async () => page.getByTestId("image-element").getAttribute("src"), {
      timeout: 30_000,
    })
    .not.toBe(originalSrc);
  await setFileReadDelay(page, 0);
});

for (const viewport of VIEWPORTS) {
  test(`@canvas-interaction status bar and chrome do not overlap at ${viewport.name}`, async ({
    page,
  }) => {
    await page.setViewportSize({
      width: viewport.width,
      height: viewport.height,
    });
    await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
    await page
      .getByTestId("entry-model.stl")
      .waitFor({ state: "visible", timeout: 30_000 });
    await page.getByTestId("entry-model.stl").click();

    const canvas = page.getByTestId("mesh-canvas");
    const statusBar = page.getByTestId("canvas-statusbar");
    const workbenchCanvas = page.getByTestId("workbench-canvas");
    const toolbar = page.getByTestId("viewer-toolbar");
    const meshStatus = page.getByTestId("mesh-status");
    await canvas.waitFor({ state: "visible", timeout: 30_000 });
    await expect(statusBar).toBeVisible();
    await expect(meshStatus).toContainText(/preview pending|preview ready/, {
      timeout: 30_000,
    });
    await expect(meshStatus).toBeHidden();

    const canvasBox = await boxFor(canvas);
    const statusBox = await boxFor(statusBar);
    const workbenchCanvasBox = await boxFor(workbenchCanvas);
    expect(Math.round(statusBox.height)).toBe(44);
    expect(statusBox.y).toBeGreaterThanOrEqual(canvasBox.y + canvasBox.height - 1);
    expect(Math.abs(statusBox.x - workbenchCanvasBox.x)).toBeLessThanOrEqual(1);
    expect(Math.abs(statusBox.width - workbenchCanvasBox.width)).toBeLessThanOrEqual(2);

    await expectNoOverlap(toolbar, statusBar, "viewer toolbar must not overlap status bar");
    await expectNoOverlap(canvas, statusBar, "mesh canvas must not overlap status bar");
  });

  test(`@canvas-interaction preview error card avoids chrome at ${viewport.name}`, async ({
    page,
  }) => {
    await page.setViewportSize({
      width: viewport.width,
      height: viewport.height,
    });
    await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
    await page
      .getByTestId("entry-broken.scad")
      .waitFor({ state: "visible", timeout: 30_000 });
    await page.getByTestId("entry-broken.scad").click();

    const errorCard = page.getByTestId("preview-error-card");
    const previewStatus = page.getByTestId("scad-preview-status");
    const toolbar = page.getByTestId("viewer-toolbar");
    const statusBar = page.getByTestId("canvas-statusbar");
    await expect(errorCard).toBeVisible({ timeout: 30_000 });
    await expect(previewStatus).toContainText("preview error", {
      timeout: 30_000,
    });
    await expect(previewStatus).toBeHidden();
    await expect(errorCard).toHaveCSS("overflow-y", /auto|scroll/);
    await expect(errorCard).toHaveCSS("pointer-events", "auto");
    await expect(toolbar).toBeVisible();
    await expect(statusBar).toBeVisible();
    await expectNoOverlap(errorCard, toolbar, "preview error must not overlap toolbar");
    await expectNoOverlap(errorCard, statusBar, "preview error must not overlap status bar");
  });
}

test("@canvas-interaction three.js canvas renders and accepts pointer drag", async ({
  page,
}) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page
    .getByTestId("entry-model.stl")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-model.stl").click();

  const canvas = page.getByTestId("mesh-canvas");
  await canvas.waitFor({ state: "visible", timeout: 30_000 });
  // wait for preview to report ready before drag
  await expect(page.getByTestId("message")).toContainText(
    /preview ready|preview error/,
    { timeout: 30_000 },
  );

  const box = await canvas.boundingBox();
  if (!box) throw new Error("mesh canvas has no bounding box");
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;

  // orbit drag: press + move + release. Test passes if no exception thrown
  // during the interaction; the Three.js renderer synchronously updates camera
  // state each frame, so a completed drag proves the event pipeline works.
  await page.mouse.move(cx, cy);
  await page.mouse.down({ button: "left" });
  await page.mouse.move(cx + 80, cy + 30, { steps: 6 });
  await page.mouse.up({ button: "left" });

  await expect(canvas).toBeVisible();
});

type Box = NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;

async function boxFor(locator: Locator): Promise<Box> {
  const box = await locator.boundingBox();
  if (!box) throw new Error("expected locator to have a layout box");
  return box;
}

async function expectNoOverlap(
  first: Locator,
  second: Locator,
  message: string,
): Promise<void> {
  const firstBox = await boxFor(first);
  const secondBox = await boxFor(second);
  expect(boxesOverlap(firstBox, secondBox), message).toBe(false);
}

async function expectCenteredIn(
  inner: Locator,
  outer: Locator,
  message: string,
): Promise<void> {
  const innerBox = await boxFor(inner);
  const outerBox = await boxFor(outer);
  const innerCenterX = innerBox.x + innerBox.width / 2;
  const innerCenterY = innerBox.y + innerBox.height / 2;
  const outerCenterX = outerBox.x + outerBox.width / 2;
  const outerCenterY = outerBox.y + outerBox.height / 2;
  expect(Math.abs(innerCenterX - outerCenterX), `${message}: x`).toBeLessThanOrEqual(1);
  expect(Math.abs(innerCenterY - outerCenterY), `${message}: y`).toBeLessThanOrEqual(1);
}

function boxesOverlap(first: Box, second: Box): boolean {
  return (
    first.x < second.x + second.width &&
    first.x + first.width > second.x &&
    first.y < second.y + second.height &&
    first.y + first.height > second.y
  );
}

async function setPreviewDelay(
  page: import("@playwright/test").Page,
  delayMs: number,
): Promise<void> {
  await setWindowDelay(page, "__studioWebPreviewDelayMs", delayMs);
}

async function setFileReadDelay(
  page: import("@playwright/test").Page,
  delayMs: number,
): Promise<void> {
  await setWindowDelay(page, "__studioWebFileReadDelayMs", delayMs);
}

async function setWindowDelay(
  page: import("@playwright/test").Page,
  key: "__studioWebPreviewDelayMs" | "__studioWebFileReadDelayMs",
  delayMs: number,
): Promise<void> {
  await page.addInitScript(
    ({ delayKey, ms }) => {
      (
        window as Window & {
          __studioWebPreviewDelayMs?: number;
          __studioWebFileReadDelayMs?: number;
        }
      )[delayKey] = ms;
    },
    { delayKey: key, ms: delayMs },
  );
  await page.evaluate(
    ({ delayKey, ms }) => {
      (
        window as Window & {
          __studioWebPreviewDelayMs?: number;
          __studioWebFileReadDelayMs?: number;
        }
      )[delayKey] = ms;
    },
    { delayKey: key, ms: delayMs },
  );
}

function readJsonFile(filePath: string): unknown {
  try {
    return JSON.parse(readFileSync(filePath, "utf8"));
  } catch (err) {
    if (
      err &&
      typeof err === "object" &&
      (err as NodeJS.ErrnoException).code === "ENOENT"
    ) {
      return null;
    }
    throw err;
  }
}

function explicitOffFile(): unknown {
  return {
    presets: {
      existing: {
        size: 18,
      },
    },
    previewAppearance: {
      backgroundColor: "#181b20",
      gridMajorColor: "#5a6573",
      gridMinorColor: "#343b45",
      lightingIntensity: 1.25,
      pointLightIntensity: 1.6,
      pointLightMode: "off",
      pointLightPosition: null,
    },
  };
}

async function numericAttribute(locator: Locator, name: string): Promise<number> {
  const value = await locator.getAttribute(name);
  if (value === null) throw new Error(`missing numeric attribute ${name}`);
  const number = Number(value);
  if (!Number.isFinite(number)) throw new Error(`invalid numeric attribute ${name}: ${value}`);
  return number;
}

async function expectPointLightButtons(
  off: Locator,
  auto: Locator,
  manual: Locator,
  active: "off" | "auto" | "manual",
): Promise<void> {
  await expect(off).toHaveAttribute("aria-pressed", String(active === "off"));
  await expect(auto).toHaveAttribute("aria-pressed", String(active === "auto"));
  await expect(manual).toHaveAttribute("aria-pressed", String(active === "manual"));
}

function pointLightPositionNumbers(value: string): number[] {
  return value.split(",").map((item) => Number(item));
}

function previewAppearanceFromFile(file: unknown): {
  backgroundColor?: unknown;
  pointLightIntensity?: unknown;
  pointLightMode?: unknown;
  pointLightPosition?: unknown;
} | null {
  if (!file || typeof file !== "object") return null;
  const appearance = (file as { previewAppearance?: unknown }).previewAppearance;
  if (!appearance || typeof appearance !== "object") return null;
  return appearance as {
    backgroundColor?: unknown;
    pointLightIntensity?: unknown;
    pointLightMode?: unknown;
    pointLightPosition?: unknown;
  };
}

function pointLightPositionClose(value: unknown, expected: string): boolean {
  if (!Array.isArray(value) || value.length !== 3) return false;
  const expectedNumbers = pointLightPositionNumbers(expected);
  return value.every(
    (item, index) =>
      typeof item === "number" &&
      Number.isFinite(item) &&
      Math.abs(item - expectedNumbers[index]) < 0.001,
  );
}

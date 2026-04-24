import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { expect, test } from "@playwright/test";
import {
  clearServiceWorkerState,
  createHarness,
  installProtocolRecorder,
} from "./_smoke-harness";

const HOST_WORKSPACE = mkdtempSync(path.join(tmpdir(), "scad-studio-invalid-entry-"));
const HARNESS = createHarness({
  bindPort: 39189,
  vitePort: 5185,
  workspacePath: HOST_WORKSPACE,
});

test.beforeAll(async () => {
  mkdirSync(path.join(HOST_WORKSPACE, "bad#dir"), { recursive: true });
  writeFileSync(path.join(HOST_WORKSPACE, "bad#file.scad"), "cube(1);\n", "utf-8");
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
  rmSync(HOST_WORKSPACE, { recursive: true, force: true });
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
  await installProtocolRecorder(page);
});

test("@invalid-entry invalid workspace entries are visible but not operable", async ({
  page,
}) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);

  const invalidDir = page.getByTestId("entry-bad#dir");
  const invalidFile = page.getByTestId("entry-bad#file.scad");
  await expect(invalidDir).toBeVisible({ timeout: 30_000 });
  await expect(invalidFile).toBeVisible();
  await expect(invalidDir).toBeDisabled();
  await expect(invalidFile).toBeDisabled();
  await expect(page.getByTestId("entry-kind-bad#dir")).toHaveText("invalid");
  await expect(page.getByTestId("entry-kind-bad#file.scad")).toHaveText("invalid");

  await invalidDir.click({ force: true });
  await invalidFile.click({ force: true });

  await expect(page.getByTestId("tabbar")).toContainText("no document open");
  await expect(page.locator('[data-testid="entries-__root__"]')).toHaveCount(0);
});

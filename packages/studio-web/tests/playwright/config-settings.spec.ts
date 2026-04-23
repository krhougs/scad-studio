// Phase 7 @config-settings smoke. Loads /settings, writes an openscad path,
// saves, reloads the page, and verifies the saved value came back via
// ConfigLoad.
//
// The host subprocess is launched with an isolated `HOME` (and Linux
// `XDG_CONFIG_HOME`) pointing at a throwaway tmp dir so `dirs::config_dir()`
// never writes to the developer's or CI's real scad-studio config. The
// directory is cleaned up when the test suite finishes.

import { mkdtempSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { clearServiceWorkerState, createHarness } from "./_smoke-harness";

const TMP_HOME = mkdtempSync(path.join(tmpdir(), "scad-studio-smoke-home-"));
const TMP_XDG = path.join(TMP_HOME, ".config");
const REAL_HOME = homedir();
// Preserve cargo / rustup caches so `cargo run` still works with the
// overridden HOME. Only `dirs::config_dir()` (which drives scad-studio's
// AppConfig location) sees the redirected paths.
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
const SENTINEL = "/tmp/smoke-openscad-" + Date.now();

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
  try {
    rmSync(TMP_HOME, { recursive: true, force: true });
  } catch {
    // best-effort cleanup; the dir lives under $TMPDIR anyway.
  }
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

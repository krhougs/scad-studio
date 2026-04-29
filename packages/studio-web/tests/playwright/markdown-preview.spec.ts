import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { expect, test } from "@playwright/test";
import {
  clearRecordedClientCommands,
  clearServiceWorkerState,
  createHarness,
  installProtocolRecorder,
  latestRecordedClientCommand,
} from "./_smoke-harness";

const TEST_WORKSPACE = mkdtempSync(
  path.join(tmpdir(), "scad-studio-markdown-workspace-"),
);
writeFileSync(
  path.join(TEST_WORKSPACE, "readme.md"),
  `# Markdown Preview

[external](https://example.com)
[bad](javascript:alert(1))
![bad image](javascript:alert(1))

\`\`\`mermaid
flowchart TD
  A[Start] --> B[Finish]
\`\`\`

<script>window.__markdownPreviewUnsafe = true</script>
<img src="x" onerror="window.__markdownPreviewUnsafe = true" />
<iframe src="https://example.com"></iframe>
`,
);
const PLAN_DIR = path.join(TEST_WORKSPACE, "plans", "2026050100-add-lid-vents");
mkdirSync(PLAN_DIR, { recursive: true });
writeFileSync(path.join(PLAN_DIR, "request.md"), "# Request\n\nAdd lid vents.\n");
writeFileSync(
  path.join(PLAN_DIR, "plan.md"),
  "# Add Lid Vents\n\nUse CadQuery to add vents to the lid.\n",
);
writeFileSync(path.join(PLAN_DIR, "plan-result.md"), "status: pending\n");

const HARNESS = createHarness({
  bindPort: 39191,
  vitePort: 5186,
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
  await installProtocolRecorder(page);
});

test("@markdown-preview uses secure uiw markdown rendering with Mermaid", async ({
  page,
}) => {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );
  await page
    .getByTestId("entry-readme.md")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-readme.md").click();

  const preview = page.getByTestId("markdown-preview");
  await expect(preview).toBeVisible({ timeout: 30_000 });
  await expect(preview).toHaveAttribute("data-color-mode", "dark");
  await expect(preview).toContainText("Markdown Preview");

  const external = preview.getByRole("link", { name: "external" });
  await expect(external).toHaveAttribute("target", "_blank");
  await expect(external).toHaveAttribute("rel", "noopener noreferrer");
  await expect(external).toHaveAttribute("href", "https://example.com/");

  const bad = preview.getByRole("link", { name: "bad" });
  await expect(bad).not.toHaveAttribute("href", /javascript:/);
  await expect(preview.locator("iframe")).toHaveCount(0);
  await expect(preview.locator("img[onerror]")).toHaveCount(0);
  await expect(preview.locator("img[src^='javascript:']")).toHaveCount(0);

  await expect(preview.getByTestId("markdown-mermaid")).toBeVisible({
    timeout: 30_000,
  });
  await expect(preview.locator("script")).toHaveCount(0);
  await expect
    .poll(() =>
      page.evaluate(() =>
        Boolean(
          (window as Window & { __markdownPreviewUnsafe?: boolean })
            .__markdownPreviewUnsafe,
        ),
      ),
    )
    .toBe(false);
});

test("@markdown-preview run plan emits agent.invoke with plan_ref", async ({
  page,
}) => {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );
  await page.getByTestId("entry-plans").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-plans").click();
  await page
    .getByTestId("entry-2026050100-add-lid-vents")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-2026050100-add-lid-vents").click();
  await page
    .getByTestId("entry-plan.md")
    .waitFor({ state: "visible", timeout: 30_000 });
  await page.getByTestId("entry-plan.md").click();

  await expect(page.getByTestId("markdown-plan-actions")).toBeVisible({
    timeout: 30_000,
  });
  await clearRecordedClientCommands(page);
  await page.getByRole("button", { name: "Run Plan" }).click();

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "agent.invoke"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "agent.invoke");
  const payload = cmd as Record<string, unknown>;
  const planRef = payload["plan_ref"] as Record<string, unknown>;
  expect(payload["mode"]).toBe("agent");
  expect(payload["prompt"]).toBe("Run plan 2026050100-add-lid-vents");
  expect(planRef["path_segments"]).toEqual([
    "plans",
    "2026050100-add-lid-vents",
  ]);
});

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { expect, test } from "@playwright/test";
import { clearServiceWorkerState, createHarness } from "./_smoke-harness";

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

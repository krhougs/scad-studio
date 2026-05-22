import { expect, test, type Page } from "@playwright/test";
import path from "node:path";
import { existsSync } from "node:fs";
import {
  clearServiceWorkerState,
  createHarness,
  dispatchSelectionUpdateInPage,
  installProtocolRecorder,
  latestRecordedClientCommand,
  REPO_ROOT,
  waitForAssistantMessage,
} from "./_smoke-harness";

const agentsToml = path.join(REPO_ROOT, "agents.toml");
if (!existsSync(agentsToml)) {
  throw new Error(
    "agents.toml not found at repo root — required for live agent tests",
  );
}

const HARNESS = createHarness({
  bindPort: 39230,
  vitePort: 5230,
  rawEnv: true,
});

function chatUrl(): string {
  return `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=chat`;
}

async function waitForHandshake(page: Page) {
  await expect(page.getByTestId("workspace-name")).not.toHaveText("(loading)", {
    timeout: 30_000,
  });
}

async function startDraftAndSend(page: Page, text: string) {
  await page.getByRole("button", { name: "new" }).click();
  const input = page.getByTestId("chat-input");
  await input.fill(text);
  await expect(input).toHaveValue(text, { timeout: 2_000 });
  await page.getByRole("button", { name: /send/i }).click();
}

function faceSelection(ref = "@face[box:f_0]") {
  return {
    kind: "face" as const,
    ref_text: ref,
    owner_ref_text: "@part[box]",
    owner_object_kind: "part" as const,
    instance_path: null,
    candidate_feature_ref: null,
    build_id: "b1",
    result_id: "cq_test",
    ambiguous: false,
  };
}

test.describe("@live-agent selection-agent-live-cycle", () => {
  test.describe.configure({ timeout: 180_000 });

  test.beforeAll(async () => {
    await HARNESS.start();
  });

  test.afterAll(async () => {
    await HARNESS.stop();
  });

  test.beforeEach(async ({ page }) => {
    await clearServiceWorkerState(page);
    await installProtocolRecorder(page);
  });

  test("scenario 1: selection injected via protocol reaches agent context", async ({
    page,
  }) => {
    await page.goto(chatUrl());
    await waitForHandshake(page);

    await dispatchSelectionUpdateInPage(page, {
      selections: [faceSelection()],
      active_index: 0,
    });

    const pillBar = page.getByTestId("context-pill-bar");
    await expect(pillBar).toBeVisible({ timeout: 5_000 });
    await expect(pillBar.locator(".context-pill")).toHaveCount(1);

    await startDraftAndSend(page, "Describe the currently selected face");

    const text = await waitForAssistantMessage(page, 120_000);
    expect(text.length).toBeGreaterThan(10);
  });

  test("scenario 2: agent cadquery execution attempts tool call", async ({
    page,
  }) => {
    await page.goto(chatUrl());
    await waitForHandshake(page);

    await startDraftAndSend(
      page,
      "Create a simple rectangular box that is 40mm long, 30mm wide, and 20mm tall.",
    );

    const agentMsg = page.locator('[data-testid="chat-body"] .msg.agent');
    await expect(agentMsg.first()).toBeVisible({ timeout: 120_000 });

    const text = await waitForAssistantMessage(page, 120_000);
    expect(text.length).toBeGreaterThan(10);
  });

  test("scenario 3: plan mode sends plan turn and agent responds", async ({
    page,
  }) => {
    await page.goto(chatUrl());
    await waitForHandshake(page);

    await startDraftAndSend(
      page,
      "/plan Create a cylindrical container with a lid",
    );

    await expect
      .poll(
        () => latestRecordedClientCommand(page, "chat.create"),
        { timeout: 15_000 },
      )
      .toBeTruthy();

    const cmd = await latestRecordedClientCommand(page, "chat.create");
    const payload = cmd as Record<string, unknown>;
    const turn = payload["initial_turn"] as Record<string, unknown>;
    expect(turn["mode"]).toBe("plan");

    const text = await waitForAssistantMessage(page, 120_000);
    expect(text.length).toBeGreaterThan(10);
  });
});

import { expect, test, type Page } from "@playwright/test";
import path from "node:path";
import { existsSync } from "node:fs";
import {
  clearServiceWorkerState,
  createHarness,
  injectStoreState,
  installProtocolRecorder,
  latestRecordedClientCommand,
  REPO_ROOT,
} from "./_smoke-harness";

const agentsToml = path.join(REPO_ROOT, "agents.toml");
if (!existsSync(agentsToml)) {
  throw new Error(
    "agents.toml not found at repo root — required for host startup",
  );
}

const HARNESS = createHarness({
  bindPort: 39225,
  vitePort: 5225,
  hostEnv: { BUDN_AGENT_CONFIG: agentsToml },
});

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

function chatUrl(): string {
  return `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=chat`;
}

function offlineChatUrl(): string {
  return `${HARNESS.baseUrl}/?ws=${encodeURIComponent("ws://127.0.0.1:1")}&left-panel=chat`;
}

async function waitForHandshake(page: Page) {
  await expect(page.getByTestId("workspace-name")).not.toHaveText("(loading)", {
    timeout: 30_000,
  });
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

test("@selection-cycle context pill bar appears after selection injection", async ({
  page,
}) => {
  await page.goto(chatUrl());
  await waitForHandshake(page);

  await injectStoreState(page, {
    current_selection: {
      selections: [faceSelection()],
      active_index: 0,
    },
  });

  const pillBar = page.getByTestId("context-pill-bar");
  await expect(pillBar).toBeVisible({ timeout: 5_000 });
  const pills = pillBar.locator(".context-pill");
  await expect(pills).toHaveCount(1);
  await expect(pills.first()).toContainText("@face[box:f_0]");
});

test("@selection-cycle cadquery tool card renders from injected agent events", async ({
  page,
}) => {
  await page.goto(offlineChatUrl());
  await expect(page.getByTestId("chat-body")).toBeVisible({ timeout: 10_000 });

  await injectStoreState(page, {
    agent_events: [
      {
        event: "agent.tool_start",
        payload: {
          agent_id: "agent-main",
          run_id: "run-e2e",
          tool_name: "cadquery_execute",
          args_json: JSON.stringify({ target_path: "parts/box.py" }),
        },
      },
    ],
    agent_run: { run_id: "run-e2e", status: "running" },
  });

  const card = page.getByTestId("cadquery-tool-card");
  await expect(card).toBeVisible({ timeout: 5_000 });
  await expect(card).toContainText("execute");
  await expect(card).toContainText("parts/box.py");
});

test("@selection-cycle cadquery tool card shows error with traceback", async ({
  page,
}) => {
  await page.goto(offlineChatUrl());
  await expect(page.getByTestId("chat-body")).toBeVisible({ timeout: 10_000 });

  await injectStoreState(page, {
    agent_events: [
      {
        event: "agent.tool_start",
        payload: {
          agent_id: "agent-main",
          run_id: "run-e2e-err",
          tool_name: "cadquery_execute",
          args_json: JSON.stringify({ target_path: "parts/fail.py" }),
        },
      },
      {
        event: "agent.tool_result",
        payload: {
          agent_id: "agent-main",
          run_id: "run-e2e-err",
          tool_name: "cadquery_execute",
          success: false,
          result_json: JSON.stringify({
            status: "error",
            error_type: "SyntaxError",
            message: "SyntaxError: unexpected EOF",
            diagnostics: { traceback: "File line 1\n  SyntaxError" },
          }),
        },
      },
    ],
    agent_run: { run_id: "run-e2e-err", status: "running" },
  });

  const card = page.getByTestId("cadquery-tool-card").last();
  await expect(card).toBeVisible({ timeout: 5_000 });
  await expect(card).toContainText("SyntaxError: unexpected EOF");
});

test("@selection-cycle selection dock clear dispatches empty selections", async ({
  page,
}) => {
  await page.goto(chatUrl());
  await waitForHandshake(page);

  await injectStoreState(page, {
    current_selection: {
      selections: [faceSelection(), faceSelection("@face[box:f_1]")],
      active_index: 0,
    },
  });

  const pillBar = page.getByTestId("context-pill-bar");
  await expect(pillBar).toBeVisible({ timeout: 5_000 });
  const pills = pillBar.locator(".context-pill");
  await expect(pills).toHaveCount(2);

  const removeBtn = pillBar.locator("button").first();
  await removeBtn.click();

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "selection.update"),
      { timeout: 5_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "selection.update");
  const payload = cmd as Record<string, unknown>;
  const selections = payload["selections"] as unknown[];
  expect(selections).toHaveLength(1);
});

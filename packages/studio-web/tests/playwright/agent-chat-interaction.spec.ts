import { expect, test } from "@playwright/test";
import {
  clearServiceWorkerState,
  createHarness,
  installProtocolRecorder,
  latestRecordedClientCommand,
  clearRecordedClientCommands,
} from "./_smoke-harness";

const HARNESS = createHarness({
  bindPort: 39220,
  vitePort: 5220,
  hostEnv: {
    BUDN_AGENT_OPENAI_API_KEY: "test-key",
    BUDN_AGENT_TIMEOUT_SECS: "1",
  },
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

async function waitForHandshake(page: import("@playwright/test").Page) {
  await expect(page.getByTestId("workspace-name")).not.toHaveText("(loading)", {
    timeout: 30_000,
  });
}

async function waitForChatReady(page: import("@playwright/test").Page) {
  await page.goto(chatUrl());
  await expect(page.getByTestId("workbench-chat")).toBeVisible({
    timeout: 30_000,
  });
}

async function waitForChatReadyWithHandshake(page: import("@playwright/test").Page) {
  await page.goto(chatUrl());
  await waitForHandshake(page);
  await expect(page.getByTestId("workbench-chat")).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByLabel("agent model")).toBeVisible({
    timeout: 10_000,
  });
}

async function startDraftChat(page: import("@playwright/test").Page) {
  await page.getByRole("button", { name: "new" }).click();
  await expect(page.getByLabel("chat session")).toHaveValue(/^draft-/, {
    timeout: 2_000,
  });
}

async function fillAndSend(
  page: import("@playwright/test").Page,
  text: string,
) {
  const input = page.getByTestId("chat-input");
  await input.fill(text);
  await expect(input).toHaveValue(text, { timeout: 2_000 });
  await page.getByRole("button", { name: /send/i }).click();
}

// --- UI-only tests (no dispatch required) ---

test("@agent-chat shows welcome empty state or chat body after handshake", async ({
  page,
}) => {
  await waitForChatReady(page);
  const emptyState = page.getByTestId("chat-empty-state");
  const chatBody = page.getByTestId("chat-body");
  const llmGuide = page.getByTestId("llm-setup-guide");
  await expect
    .poll(
      async () => {
        const visible = await Promise.all([
          emptyState.isVisible().catch(() => false),
          chatBody.isVisible().catch(() => false),
          llmGuide.isVisible().catch(() => false),
        ]);
        return visible.some(Boolean);
      },
      { timeout: 10_000 },
    )
    .toBe(true);
});

test("@agent-chat llm status dot element exists in chat header", async ({
  page,
}) => {
  await waitForChatReady(page);
  const header = page.locator(".chat-head");
  await expect(header).toBeVisible({ timeout: 10_000 });
  await expect(header.locator(".llm-dot")).toHaveCount(1);
  await expect(header.locator(".llm-dot")).toHaveAttribute("title", /(AI connected|AI not configured)/);
});

test("@agent-chat navigating to chat panel via rail button", async ({
  page,
}) => {
  await page.goto(
    `${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`,
  );
  await expect(page.getByTestId("workbench-left-panel")).toBeVisible({
    timeout: 30_000,
  });

  await page.getByTestId("rail-chat").click();
  await expect(page).toHaveURL(/left-panel=chat/);
  await expect(page.getByTestId("workbench-chat")).toBeVisible({
    timeout: 10_000,
  });
});

test("@agent-chat input area renders with placeholder and send button", async ({
  page,
}) => {
  await waitForChatReady(page);
  const input = page.getByTestId("chat-input");
  await expect(input).toBeVisible({ timeout: 10_000 });
  await expect(input).toHaveAttribute(
    "placeholder",
    /describe what you want/i,
  );
  await expect(page.getByRole("button", { name: /send/i })).toBeVisible();
});

test("@agent-chat mode selector only exposes Agent and Plan", async ({
  page,
}) => {
  await waitForChatReady(page);
  const options = await page
    .getByLabel("agent mode")
    .locator("option")
    .allTextContents();
  expect(options).toEqual(["Agent", "Plan"]);
});

// --- Protocol frame tests (require completed handshake) ---

test("@agent-chat first message emits chat.create initial_turn with mode agent", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await startDraftChat(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "make the lid taller");

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "chat.create"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "chat.create");
  const payload = cmd as Record<string, unknown>;
  const turn = payload["initial_turn"] as Record<string, unknown>;
  expect(payload["initial_user_message"]).toBe("make the lid taller");
  expect(turn["mode"]).toBe("agent");
  expect(turn["plan_ref"]).toBeFalsy();
});

test("@agent-chat slash command /plan sends mode plan in chat.create initial_turn", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await startDraftChat(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "/plan design a sliding lid mechanism");

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
  expect(payload["initial_user_message"]).toBe("design a sliding lid mechanism");
});

test("@agent-chat slash command /agent sends mode agent", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await startDraftChat(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "/agent apply the changes");

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "chat.create"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "chat.create");
  const payload = cmd as Record<string, unknown>;
  const turn = payload["initial_turn"] as Record<string, unknown>;
  expect(turn["mode"]).toBe("agent");
  expect(payload["initial_user_message"]).toBe("apply the changes");
});

test("@agent-chat slash command /execute is sent as normal agent text", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await startDraftChat(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "/execute apply the changes");

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "chat.create"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "chat.create");
  const payload = cmd as Record<string, unknown>;
  const turn = payload["initial_turn"] as Record<string, unknown>;
  expect(turn["mode"]).toBe("agent");
  expect(payload["initial_user_message"]).toBe("/execute apply the changes");
});

test("@agent-chat first create carries the display content without slash prefix", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await startDraftChat(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "/plan design a box");

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "chat.create"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "chat.create");
  const payload = cmd as Record<string, unknown>;
  expect(payload["initial_user_message"]).toBe("design a box");
});

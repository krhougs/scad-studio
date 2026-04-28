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
  await expect(emptyState.or(chatBody).or(llmGuide)).toBeVisible({
    timeout: 10_000,
  });
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

// --- Protocol frame tests (require completed handshake) ---

test("@agent-chat sending a message emits agent.invoke with operation auto", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "make the lid taller");

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "agent.invoke"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "agent.invoke");
  const payload = cmd as Record<string, unknown>;
  expect(payload["operation"]).toBe("auto");
  expect(payload["prompt"]).toBe("make the lid taller");
  expect(payload["confirmed_cadquery"]).toBeFalsy();
});

test("@agent-chat slash command /plan sends operation plan in protocol frame", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "/plan design a sliding lid mechanism");

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "agent.invoke"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "agent.invoke");
  const payload = cmd as Record<string, unknown>;
  expect(payload["operation"]).toBe("plan");
  expect(payload["prompt"]).toBe("design a sliding lid mechanism");
});

test("@agent-chat slash command /execute sends operation execute", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "/execute apply the changes");

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "agent.invoke"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "agent.invoke");
  const payload = cmd as Record<string, unknown>;
  expect(payload["operation"]).toBe("execute");
  expect(payload["prompt"]).toBe("apply the changes");
});

test("@agent-chat slash command /inform sends operation inform", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "/inform explain CadQuery fillet");

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "agent.invoke"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "agent.invoke");
  const payload = cmd as Record<string, unknown>;
  expect(payload["operation"]).toBe("inform");
  expect(payload["prompt"]).toBe("explain CadQuery fillet");
});

test("@agent-chat chat.send frame carries the display content without slash prefix", async ({
  page,
}) => {
  await waitForChatReadyWithHandshake(page);
  await clearRecordedClientCommands(page);

  await fillAndSend(page, "/plan design a box");

  await expect
    .poll(
      () => latestRecordedClientCommand(page, "chat.send"),
      { timeout: 15_000 },
    )
    .toBeTruthy();

  const cmd = await latestRecordedClientCommand(page, "chat.send");
  const payload = cmd as Record<string, unknown>;
  expect(payload["content"]).toBe("design a box");
});

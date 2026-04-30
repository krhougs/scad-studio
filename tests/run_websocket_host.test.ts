import { expect, test } from "bun:test";
import path from "node:path";
import {
  DEFAULT_WORKSPACE,
  REPO_ROOT,
  waitForHostReady,
} from "../scripts/run_websocket_host";

test("default websocket host workspace uses neutral budn workspace", () => {
  expect(DEFAULT_WORKSPACE).toBe(path.join(REPO_ROOT, "workspace", "budn-web"));
});

test("waitForHostReady rejects when host exits before port is ready", async () => {
  await expect(
    waitForHostReady(Promise.resolve(42), new Promise(() => {})),
  ).rejects.toThrow("websocket host exited before becoming ready: 42");
});

test("waitForHostReady resolves when port becomes ready first", async () => {
  await waitForHostReady(new Promise(() => {}), Promise.resolve());
});

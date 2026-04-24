import { useSyncExternalStore } from "react";
import type { AppConfigShape, AppConfigState } from "./app-config";

let state: AppConfigState = { kind: "idle" };
const listeners = new Set<() => void>();

export function useAppConfigState(): AppConfigState {
  return useSyncExternalStore(subscribe, getAppConfigState, getAppConfigState);
}

export function getAppConfigState(): AppConfigState {
  return state;
}

export function setAppConfigLoading(): void {
  state = { kind: "loading" };
  emit();
}

export function setAppConfigReady(
  config: AppConfigShape,
  raw: string,
  source: "load" | "save",
): void {
  state = { kind: "ready", config, raw, source };
  emit();
}

export function setAppConfigError(message: string): void {
  state = { kind: "error", message };
  emit();
}

export function resetAppConfigState(): void {
  state = { kind: "idle" };
  emit();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function emit(): void {
  for (const listener of listeners) {
    listener();
  }
}

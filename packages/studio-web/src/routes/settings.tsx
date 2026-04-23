// Settings route: loads AppConfig JSON from the server, exposes the most
// relevant fields (OpenSCAD path, workspaces, slicers) as editable inputs,
// and saves the mutation back through ConfigSave. AppConfig itself is a
// protocol payload, so it lives in React useState here — the Zustand UI store
// keeps its protocol-state-free boundary (see CLAUDE.md / plan-00).

import { useCallback, useEffect, useMemo, useState } from "react";
import { BrowserWebSocketTransport } from "../transport/websocket-transport";
import { WasmClient } from "../wasm-bridge";
import { buildHandshakeParams } from "../workbench/workbench-wiring";
import { describeFileReadError } from "../viewers/file-read-decoder";

type SlicerRow = { name: string; path: string };

type AppConfigShape = {
  openscad_path?: string | null;
  slicers?: SlicerRow[];
  recent_workspaces?: string[];
  floating_panel_opacity?: number;
  camera_overlay_pos?: [number, number] | null;
  camera_overlay_size?: [number, number] | null;
  param_panel_pos?: [number, number] | null;
  param_panel_size?: [number, number] | null;
  log_panel_pos?: [number, number] | null;
  log_panel_size?: [number, number] | null;
};

type LoadState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; config: AppConfigShape; raw: string }
  | { kind: "error"; message: string };

export function SettingsRoute() {
  const wsUrl = useMemo(() => {
    const search = new URLSearchParams(window.location.search);
    return search.get("ws") ?? "ws://127.0.0.1:38421";
  }, []);
  const [state, setState] = useState<LoadState>({ kind: "idle" });
  const [openscadPath, setOpenscadPath] = useState<string>("");
  const [saveStatus, setSaveStatus] = useState<string>("");
  const [client, setClient] = useState<WasmClient | null>(null);

  useEffect(() => {
    const wasmClient = new WasmClient({
      onSnapshotDirty: () => {},
      onHandshakeAccepted: () => {
        setState({ kind: "loading" });
        wasmClient
          .dispatchConfigLoad()
          .then((response) => {
            const decoded = decodeConfigLoad(response);
            setState({ kind: "ready", config: decoded.config, raw: decoded.raw });
            setOpenscadPath(decoded.config.openscad_path ?? "");
          })
          .catch((err) => {
            setState({ kind: "error", message: describeFileReadError(err) });
          });
      },
    });
    setClient(wasmClient);
    const transport = new BrowserWebSocketTransport(wsUrl, {
      onOpen: () => {
        wasmClient.setSender((bytes) => transport.send(bytes));
        wasmClient.beginHandshake(buildHandshakeParams());
        wasmClient.pump();
      },
      onMessage: (bytes) => {
        wasmClient.receiveInbound(bytes);
      },
      onClose: (reason) => {
        wasmClient.setSender(null);
        wasmClient.markTransportClosed(reason);
      },
      onError: (msg) => {
        setState({ kind: "error", message: msg });
      },
    });
    transport.start();
    const tick = window.setInterval(() => wasmClient.pump(), 200);
    return () => {
      window.clearInterval(tick);
      transport.stop();
      wasmClient.destroy();
    };
  }, [wsUrl]);

  const save = useCallback(async () => {
    if (!client) return;
    if (state.kind !== "ready") return;
    setSaveStatus("saving");
    const next: AppConfigShape = {
      ...state.config,
      openscad_path: openscadPath.trim() ? openscadPath.trim() : null,
    };
    try {
      await client.dispatchConfigSave({ json: JSON.stringify(next) });
      setSaveStatus("saved");
      setState({ kind: "ready", config: next, raw: JSON.stringify(next) });
    } catch (err) {
      setSaveStatus(`save error: ${describeFileReadError(err)}`);
    }
  }, [client, state, openscadPath]);

  return (
    <section
      className="settings"
      style={{ padding: "16px", fontFamily: "ui-monospace, monospace" }}
      data-testid="settings-route"
    >
      <h1>settings</h1>
      {state.kind === "loading" ? (
        <p data-testid="settings-loading">loading config…</p>
      ) : null}
      {state.kind === "error" ? (
        <p data-testid="settings-error">error: {state.message}</p>
      ) : null}
      {state.kind === "ready" ? (
        <div className="settings__body">
          <label style={{ display: "block", marginTop: "12px" }}>
            openscad path
            <input
              type="text"
              value={openscadPath}
              onChange={(ev) => setOpenscadPath(ev.target.value)}
              data-testid="settings-openscad-path"
              style={{ width: "100%", marginTop: "4px" }}
            />
          </label>
          <p>
            recent workspaces:{" "}
            <span data-testid="settings-recent-count">
              {(state.config.recent_workspaces ?? []).length}
            </span>
          </p>
          <p>
            configured slicers:{" "}
            <span data-testid="settings-slicer-count">
              {(state.config.slicers ?? []).length}
            </span>
          </p>
          <button
            type="button"
            onClick={save}
            data-testid="settings-save"
            style={{ marginTop: "12px" }}
          >
            save
          </button>
          <p data-testid="settings-status">{saveStatus || "idle"}</p>
        </div>
      ) : null}
      <p style={{ marginTop: "24px", color: "#888" }}>
        WebSocket URL: <code>{wsUrl}</code>
      </p>
    </section>
  );
}

function decodeConfigLoad(response: unknown): {
  config: AppConfigShape;
  raw: string;
} {
  const outer = (response ?? {}) as Record<string, unknown>;
  const inner = (outer["payload"] as Record<string, unknown> | undefined) ?? outer;
  const json = inner["json"];
  if (typeof json !== "string") {
    throw new Error("ConfigLoadResponse.json missing");
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    throw new Error("ConfigLoadResponse.json is not valid JSON");
  }
  if (!parsed || typeof parsed !== "object") {
    throw new Error("config is not an object");
  }
  return { config: parsed as AppConfigShape, raw: json };
}

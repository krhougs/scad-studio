import {
  initProtocolWasm,
  protocol_decode_client_frame,
  protocol_decode_server_frame,
  protocol_encode_agent_invoke_request,
  protocol_encode_config_save_request,
  protocol_encode_export_run_request,
  protocol_path_handle,
  protocol_resolve_relative_link,
  protocol_validate_config,
  protocol_validate_finite_number,
  protocol_validate_host_local_path,
  type AppConfigDto,
  type AgentInvokeRequest,
  type ConfigSaveRequest,
  type ExportRunRequest,
  type PathHandle,
} from "../../packages/app-server-protocol/src/index";

await initProtocolWasm();

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function assertProtocolError(error: unknown, code: string): void {
  const record = error as { code?: unknown; message?: unknown };
  assert(record.code === code, `expected ${code}, got ${String(record.code)}`);
  assert(typeof record.message === "string", `${code} error missing message`);
}

const path = protocol_path_handle("ws", ["零件", "支架👍🏽.scad"]) as PathHandle;
assert(path.path_segments.join("/") === "零件/支架👍🏽.scad", "CJK / emoji path failed");

const jsPath = protocol_path_handle("ws", ["src", "routes", "+page.svelte"]) as PathHandle;
assert(jsPath.path_segments.at(-1) === "+page.svelte", "JS route filename failed");

const base = protocol_path_handle("ws", ["docs", "guide.md"]) as PathHandle;
const linked = protocol_resolve_relative_link(base, "../模型/%E9%9B%B6件.scad") as PathHandle;
assert(linked.path_segments.join("/") === "模型/零件.scad", "relative link failed");

try {
  protocol_path_handle("ws", ["CON.scad"]);
  throw new Error("reserved name was accepted");
} catch (error) {
  assertProtocolError(error, "windows_reserved_name");
}

try {
  protocol_resolve_relative_link(base, "../../secret.scad");
  throw new Error("escaping relative link was accepted");
} catch (error) {
  assertProtocolError(error, "relative_path_escapes_root");
}

protocol_validate_host_local_path(String.raw`C:\Program Files\OpenSCAD\openscad.exe`);
protocol_validate_host_local_path("/Applications/OpenSCAD.app/Contents/MacOS/OpenSCAD");
protocol_validate_host_local_path("/usr/bin/openscad");

const config: AppConfigDto = {
  openscad_path: "/usr/bin/openscad",
  slicers: [{ name: "PrusaSlicer", path: "/usr/bin/prusa-slicer" }],
  recent_workspaces: ["/tmp/ws"],
  floating_panel_opacity: 0.85,
  left_panel_width: 360,
  right_panel_width: 320,
  display_unit: "millimeter",
  camera_overlay_pos: null,
  camera_overlay_size: null,
  param_panel_pos: null,
  param_panel_size: null,
  log_panel_pos: null,
  log_panel_size: null,
};
const frame = protocol_validate_config(config);
assert(frame instanceof Uint8Array && frame.length > 5, "config DTO frame failed");
const decodedConfig = protocol_decode_server_frame(frame) as Record<string, unknown>;
assert(JSON.stringify(decodedConfig).includes("config_loaded"), "server frame decode failed");

const configSaveRequest: ConfigSaveRequest = { config };
const configSaveFrame = protocol_encode_config_save_request(6n, configSaveRequest);
assert(configSaveFrame instanceof Uint8Array && configSaveFrame.length > 5, "config.save frame failed");
const decodedConfigSave = protocol_decode_client_frame(configSaveFrame) as Record<string, unknown>;
assert(JSON.stringify(decodedConfigSave).includes("config.save"), "config.save decode failed");

protocol_validate_finite_number(1.25, "camera_overlay_pos");
try {
  protocol_validate_finite_number(Number.POSITIVE_INFINITY, "camera_overlay_pos");
  throw new Error("infinite number was accepted");
} catch (error) {
  assertProtocolError(error, "invalid_numeric_value");
}

const exportRequest: ExportRunRequest = {
  configured_openscad_path: "/usr/bin/openscad",
  configured_slicers: [{ name: "PrusaSlicer", path: "/usr/bin/prusa-slicer" }],
  source: path,
  defines: ["width=12"],
  output_path: protocol_path_handle("ws", ["零件", "支架.3mf"]) as PathHandle,
  format: "three_mf",
  slicer_name: "PrusaSlicer",
};
const exportFrame = protocol_encode_export_run_request(7n, exportRequest);
assert(exportFrame instanceof Uint8Array && exportFrame.length > 5, "export frame failed");
const decodedExport = protocol_decode_client_frame(exportFrame) as Record<string, unknown>;
assert(JSON.stringify(decodedExport).includes("export.run"), "export request decode failed");

const agentInvokeRequest: AgentInvokeRequest = {
  session_id: "chat-1",
  prompt: "run plan",
  mode: "agent",
  plan_ref: protocol_path_handle("ws", ["plans", "2026050100-add-lid-vents"]) as PathHandle,
  context_refs: ["@part[top_lid]"],
};
const agentInvokeFrame = protocol_encode_agent_invoke_request(8n, agentInvokeRequest);
assert(
  agentInvokeFrame instanceof Uint8Array && agentInvokeFrame.length > 5,
  "agent.invoke frame failed",
);
const decodedAgentInvoke = protocol_decode_client_frame(agentInvokeFrame) as Record<
  string,
  unknown
>;
assert(JSON.stringify(decodedAgentInvoke).includes("agent.invoke"), "agent.invoke decode failed");
assert(JSON.stringify(decodedAgentInvoke).includes("plan_ref"), "agent.invoke plan_ref missing");

console.log("[protocol-package] smoke OK");

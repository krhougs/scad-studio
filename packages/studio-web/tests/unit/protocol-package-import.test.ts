import { describe, expect, it } from "vitest";
import {
  initProtocolWasm,
  protocol_decode_server_frame,
  type AppConfigDto,
} from "@budn/app-server-protocol";

describe("protocol package import", () => {
  it("exposes wasm init and shared config DTO types", () => {
    const config: AppConfigDto = {
      openscad_path: null,
      slicers: [],
      recent_workspaces: [],
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

    expect(typeof initProtocolWasm).toBe("function");
    expect(typeof protocol_decode_server_frame).toBe("function");
    expect(config.display_unit).toBe("millimeter");
  });
});

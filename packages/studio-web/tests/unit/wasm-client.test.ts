import { describe, expect, it, vi } from "vitest";
import * as Wasm from "@budn/studio-web-wasm";
import { WasmClient } from "../../src/wasm-bridge";

describe("WasmClient", () => {
  it("resolves cadquery result get as a lightweight payload and leaves mesh in side buffer", async () => {
    const onSnapshotDirty = vi.fn();
    const client = new WasmClient({ onSnapshotDirty });
    const testWasm = Wasm as unknown as WasmStubControls;
    const mesh = { metadata: () => ({ result_id: "cq_abc" }) } as Wasm.CadQueryMeshHandle;
    testWasm.__setCadQueryMesh(mesh);
    testWasm.__queueClientEvents([
      {
        type: "request_succeeded",
        payload: {
          request_id: 77n,
          payload: {
            type: "cad_query_result_ready",
            payload: { result_id: "cq_abc" },
          },
        },
      },
    ]);

    const result = await client.dispatchCadQueryResultGet({ result_id: "cq_abc" });

    expect(result).toEqual({
      type: "cad_query_result_ready",
      payload: { result_id: "cq_abc" },
    });
    expect(client.takeCadQueryMesh("cq_abc")).toBe(mesh);
    expect(onSnapshotDirty).toHaveBeenCalled();
  });

  it("dispatches agent model registry commands through the wasm bridge", () => {
    const client = new WasmClient({ onSnapshotDirty: vi.fn() });
    const registrySpy = vi.spyOn(Wasm, "client_dispatch_agent_model_registry");
    const selectSpy = vi.spyOn(Wasm, "client_dispatch_agent_model_select");
    const paramsSpy = vi.spyOn(Wasm, "client_dispatch_agent_model_params_update");

    void client.dispatchAgentModelRegistry();
    void client.dispatchAgentModelSelect({
      provider_id: "openai",
      model_id: "gpt-5.2",
    });
    void client.dispatchAgentModelParamsUpdate({
      provider_id: "openai",
      model_id: "gpt-5.2",
      reasoning_effort: "high",
      service_label: "flex",
    });

    expect(registrySpy).toHaveBeenCalled();
    expect(selectSpy).toHaveBeenCalledWith(
      expect.any(Wasm.ClientHandle),
      expect.objectContaining({ provider_id: "openai", model_id: "gpt-5.2" }),
    );
    expect(paramsSpy).toHaveBeenCalledWith(
      expect.any(Wasm.ClientHandle),
      expect.objectContaining({
        reasoning_effort: "high",
        service_label: "flex",
      }),
    );
  });
});

type WasmStubControls = typeof Wasm & {
  __setCadQueryMesh: (mesh: Wasm.CadQueryMeshHandle | undefined) => void;
  __queueClientEvents: (events: unknown[]) => void;
};

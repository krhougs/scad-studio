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
});

type WasmStubControls = typeof Wasm & {
  __setCadQueryMesh: (mesh: Wasm.CadQueryMeshHandle | undefined) => void;
  __queueClientEvents: (events: unknown[]) => void;
};

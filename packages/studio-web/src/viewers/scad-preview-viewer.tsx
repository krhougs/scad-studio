import type { CameraPreset } from "../canvas/camera-state";
import { WasmClient } from "../wasm-bridge";
import { MeshViewer } from "./mesh-viewer";
import type { MeshViewerOptions } from "./viewer-options";

type ScadPreviewViewerProps = {
  path: unknown;
  client: WasmClient;
  label: string;
  defines?: string[];
  configuredOpenscadPath?: string | null;
  cameraPreset?: CameraPreset | null;
  viewerOptions?: MeshViewerOptions;
  previewEnabled?: boolean;
  refreshSignal?: number;
  onPreviewStatus?: (status: string) => void;
  onStats?: (stats: { vertices: number; indices: number } | null) => void;
};

export function ScadPreviewViewer({
  path,
  client,
  label,
  defines,
  configuredOpenscadPath,
  cameraPreset,
  viewerOptions,
  previewEnabled,
  refreshSignal,
  onPreviewStatus,
  onStats,
}: ScadPreviewViewerProps) {
  return (
    <div
      className="viewer viewer--scad-preview"
      data-testid="scad-preview-viewer"
      data-label={label}
    >
      <MeshViewer
        path={path}
        client={client}
        label={label}
        defines={defines}
        configuredOpenscadPath={configuredOpenscadPath}
        cameraPreset={cameraPreset}
        viewerOptions={viewerOptions}
        previewEnabled={previewEnabled}
        refreshSignal={refreshSignal}
        statusTestId="scad-preview-status"
        onPreviewStatus={onPreviewStatus}
        onStats={onStats}
      />
    </div>
  );
}

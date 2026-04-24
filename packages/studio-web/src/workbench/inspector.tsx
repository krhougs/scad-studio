import {
  configuredSlicerRecords,
  describeConfigGaps,
  type AppConfigState,
} from "../config/app-config";
import type React from "react";
import type { WasmClient } from "../wasm-bridge";
import { ExportPanel } from "./export-panel";
import { InspectorSection } from "./inspector-section";
import { SlicerPanel } from "./slicer-panel";
import type { MeshInfo } from "../viewers/mesh-info";

export type InspectorMeshSummary = {
  label: string;
} & MeshInfo;

type InspectorProps = {
  rootName: string;
  previewTargetLabel: string;
  meshSummary: InspectorMeshSummary | null;
  client?: WasmClient | null;
  showMeshPanels?: boolean;
  meshSource?: unknown;
  defaultExportFilename?: string;
  exportDefines?: string[];
  appConfig: AppConfigState;
  onExportStatus?: (status: string) => void;
  cameraSlot?: React.ReactNode;
  parametersSlot?: React.ReactNode;
  presetsSlot?: React.ReactNode;
};

export function Inspector(props: InspectorProps) {
  const {
    rootName,
    previewTargetLabel,
    meshSummary,
    client,
    showMeshPanels,
    meshSource,
    defaultExportFilename,
    exportDefines,
    appConfig,
    onExportStatus,
    cameraSlot,
    parametersSlot,
    presetsSlot,
  } = props;
  const readyConfig = appConfig.kind === "ready" ? appConfig.config : null;
  const displayUnit = readyConfig?.display_unit ?? "millimeter";
  const configGaps = readyConfig ? describeConfigGaps(readyConfig) : [];
  const exportSlicers = readyConfig ? configuredSlicerRecords(readyConfig) : [];

  return (
    <aside
      className="inspector"
      aria-label="inspector"
      data-testid="workbench-inspector"
    >
      <header className="inspector-head">
        <div className="kicker">§ inspector</div>
        <div className="title">{rootName}</div>
      </header>
      <div className="insp-body">
        <InspectorSection id="preview" title="preview">
          <PreviewSummary
            previewTargetLabel={previewTargetLabel}
            meshSummary={meshSummary}
            displayUnit={displayUnit}
          />
        </InspectorSection>
        <InspectorSection id="config" title="config">
          <ConfigSummary
            appConfig={appConfig}
            readyConfig={readyConfig}
            configGaps={configGaps}
            exportSlicers={exportSlicers}
          />
        </InspectorSection>
        {cameraSlot ? (
          <InspectorSection id="camera" title="camera">
            {cameraSlot}
          </InspectorSection>
        ) : null}
        {parametersSlot ? (
          <InspectorSection id="parameters" title="parameters">
            {parametersSlot}
          </InspectorSection>
        ) : null}
        {presetsSlot ? (
          <InspectorSection id="presets" title="presets">
            {presetsSlot}
          </InspectorSection>
        ) : null}
        {showMeshPanels && client && meshSource !== undefined ? (
          <>
            <InspectorSection id="export" title="export">
              <ExportPanel
                client={client}
                source={meshSource}
                defaultFilename={defaultExportFilename ?? "export.stl"}
                defines={exportDefines}
                configuredOpenscadPath={readyConfig?.openscad_path ?? null}
                configuredSlicers={exportSlicers}
                onStatus={onExportStatus ?? (() => {})}
              />
            </InspectorSection>
            <InspectorSection id="slicers" title="slicers">
              <SlicerPanel
                client={client}
                source={meshSource}
                defaultFilename={defaultExportFilename}
                defines={exportDefines}
                config={readyConfig}
                onStatus={onExportStatus}
              />
            </InspectorSection>
          </>
        ) : null}
      </div>
    </aside>
  );
}

function PreviewSummary({
  previewTargetLabel,
  meshSummary,
  displayUnit,
}: {
  previewTargetLabel: string;
  meshSummary: InspectorMeshSummary | null;
  displayUnit: "millimeter" | "centimeter" | "inch";
}) {
  return (
    <>
      <div className="field">
        <div className="field-label">
          <span>active target</span>
        </div>
        <div
          className={`field-status${previewTargetLabel === "—" ? "" : " is-ok"}`}
          data-testid="preview-target"
        >
          {previewTargetLabel}
        </div>
      </div>
      {meshSummary ? (
        <div className="field">
          <div className="field-label">
            <span>mesh</span>
          </div>
          <div className="field-status is-ok" data-testid="preview-mesh-summary">
            {meshSummary.vertices} verts · {meshSummary.indices} idx
          </div>
        </div>
      ) : null}
      {meshSummary ? (
        <div className="field">
          <div className="field-label">
            <span>size</span>
          </div>
          <div className="field-status is-ok" data-testid="preview-mesh-size">
            {formatDimensions(meshSummary.dimensions, displayUnit)}
          </div>
        </div>
      ) : null}
    </>
  );
}

function formatDimensions(
  dimensions: [number, number, number],
  unit: "millimeter" | "centimeter" | "inch",
): string {
  const factor = unit === "centimeter" ? 0.1 : unit === "inch" ? 1 / 25.4 : 1;
  const label = unit === "centimeter" ? "cm" : unit === "inch" ? "in" : "mm";
  return `${dimensions
    .map((value) => (value * factor).toFixed(unit === "millimeter" ? 2 : 3))
    .join(" × ")} ${label}`;
}

function ConfigSummary({
  appConfig,
  readyConfig,
  configGaps,
  exportSlicers,
}: {
  appConfig: AppConfigState;
  readyConfig: Extract<AppConfigState, { kind: "ready" }>["config"] | null;
  configGaps: string[];
  exportSlicers: Array<{ name: string; path: string }>;
}) {
  return (
    <>
      <div className="field">
        <div className="field-label">
          <span>status</span>
        </div>
        <div className="field-status" data-testid="config-status">
          {appConfig.kind === "idle"
            ? "idle"
            : appConfig.kind === "loading"
              ? "loading config…"
              : appConfig.kind === "error"
                ? `config error: ${appConfig.message}`
                : configGaps.length > 0
                  ? `config incomplete: ${configGaps.join(", ")}`
                  : "config ready"}
        </div>
      </div>
      {readyConfig ? (
        <>
          <div className="field">
            <div className="field-label">
              <span>openscad</span>
            </div>
            <div className="field-status" data-testid="config-openscad-path">
              {readyConfig.openscad_path ?? "—"}
            </div>
          </div>
          <div className="field">
            <div className="field-label">
              <span>slicers</span>
            </div>
            <div className="field-status" data-testid="config-slicer-count">
              {exportSlicers.length}
            </div>
          </div>
        </>
      ) : null}
    </>
  );
}

// Export panel: inline form that fires ExportRun for the currently active
// mesh or scad tab. Web clients cannot know the server's absolute filesystem
// paths, so we pass a relative filename; the server's OpenSCAD CLI resolves
// it against its working directory (see docs/known_issues.md).

import { useState } from "react";
import { WasmClient } from "../wasm-bridge";
import { describeFileReadError } from "../viewers/file-read-decoder";

type ExportPanelProps = {
  client: WasmClient | null;
  source: unknown;
  defaultFilename: string;
  onStatus: (status: string) => void;
};

type ExportFormat = "stl" | "three_mf";

type LocalState = "idle" | "running" | "done" | "error";

export function ExportPanel({
  client,
  source,
  defaultFilename,
  onStatus,
}: ExportPanelProps) {
  const [format, setFormat] = useState<ExportFormat>("stl");
  const [filename, setFilename] = useState(defaultFilename);
  const [state, setState] = useState<LocalState>("idle");
  const [lastMessage, setLastMessage] = useState<string>("");

  const disabled = !client || state === "running";

  const doExport = async () => {
    if (!client) return;
    const effective = filename.trim() || defaultFilename;
    setState("running");
    setLastMessage("export running");
    onStatus("export running");
    try {
      await client.dispatchExportRun({
        configured_openscad_path: null,
        configured_slicers: [],
        source,
        defines: [],
        output_path: effective,
        format,
        slicer_name: null,
      });
      setState("done");
      setLastMessage(`export done: ${effective}`);
      onStatus(`export done: ${effective}`);
    } catch (err) {
      const message = describeFileReadError(err);
      setState("error");
      setLastMessage(`export error: ${message}`);
      onStatus(`export error: ${message}`);
    }
  };

  return (
    <section
      className="panel panel--export"
      aria-label="export"
      data-testid="export-panel"
    >
      <header className="panel__head">
        <h5 className="panel__title">export</h5>
        <span className="panel__sub" data-testid="export-status">
          {lastMessage || "idle"}
        </span>
      </header>
      <div className="panel__row panel__row--compose">
        <label className="panel__field">
          <span className="panel__field-label">format</span>
          <select
            className="panel__input"
            value={format}
            onChange={(ev) => setFormat(ev.target.value as ExportFormat)}
            data-testid="export-format"
          >
            <option value="stl">stl</option>
            <option value="three_mf">3mf</option>
          </select>
        </label>
        <label className="panel__field">
          <span className="panel__field-label">filename</span>
          <input
            type="text"
            className="panel__input"
            value={filename}
            onChange={(ev) => setFilename(ev.target.value)}
            placeholder={defaultFilename}
            data-testid="export-filename"
          />
        </label>
        <button
          type="button"
          className="btn btn--solid btn--sm"
          disabled={disabled}
          onClick={doExport}
          data-testid="export-run"
        >
          export
        </button>
      </div>
    </section>
  );
}

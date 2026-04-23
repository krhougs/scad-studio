// Inspector: workspace 目录条目与选中文件元数据。Phase 4 的 inspector 仍是
// Phase 3 端到端流程的“结果展示 + 预览触发”出口；真实参数/预设/导出在后续 phase。

export type InspectorEntry = {
  label: string;
  path: unknown;
  kind: "file" | "directory";
};

type InspectorProps = {
  rootName: string;
  entries: InspectorEntry[];
  onRequestPreview: (path: unknown) => void;
  previewTargetLabel: string;
};

export function Inspector({
  rootName,
  entries,
  onRequestPreview,
  previewTargetLabel,
}: InspectorProps) {
  return (
    <aside
      className="workbench__inspector"
      aria-label="inspector"
      data-testid="workbench-inspector"
    >
      <header className="inspector__head">
        <div className="inspector__kicker">§ inspector</div>
        <div className="inspector__title">{rootName}</div>
      </header>
      <div className="inspector__body">
        <section className="inspector__section" data-testid="inspector-entries">
          <h5>entries</h5>
          {entries.length === 0 ? (
            <p className="chat__placeholder">workspace entries loading…</p>
          ) : (
            <ul className="inspector__list" data-testid="entries">
              {entries.map((entry, idx) => (
                <li
                  key={`${entry.label}-${idx}`}
                  className="inspector__list-item"
                >
                  <button
                    type="button"
                    className="btn btn--ghost btn--sm"
                    onClick={() => onRequestPreview(entry.path)}
                    data-testid={`entry-${entry.label}`}
                    title={`preview ${entry.label}`}
                  >
                    {entry.label}
                  </button>
                  <span className="dim">{entry.kind}</span>
                </li>
              ))}
            </ul>
          )}
        </section>
        <section className="inspector__section">
          <h5>preview target</h5>
          <div className="inspector__field">
            <span className="inspector__field-label">active</span>
            <span
              className={`inspector__field-value${previewTargetLabel === "—" ? " is-muted" : ""}`}
              data-testid="preview-target"
            >
              {previewTargetLabel}
            </span>
          </div>
        </section>
      </div>
    </aside>
  );
}

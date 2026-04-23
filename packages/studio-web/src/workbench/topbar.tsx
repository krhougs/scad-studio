// Topbar: brand + workspace breadcrumb + connection status.
// 设计规范见 docs/design-system/studio-datasheet-workbench.md §六。

export type TopbarStatus = "idle" | "connecting" | "ready" | "busy" | "error";

type TopbarProps = {
  workspaceName: string;
  wsUrl: string;
  status: TopbarStatus;
  message: string;
};

function statusText(status: TopbarStatus): string {
  switch (status) {
    case "idle":
      return "offline";
    case "connecting":
      return "connecting";
    case "ready":
      return "online";
    case "busy":
      return "working";
    case "error":
      return "error";
  }
}

function statusDotClass(status: TopbarStatus): string {
  if (status === "ready") return "dot";
  if (status === "busy") return "dot dot--live";
  if (status === "error") return "dot dot--err";
  return "dot";
}

export function Topbar({ workspaceName, wsUrl, status, message }: TopbarProps) {
  return (
    <header className="workbench__topbar" data-testid="workbench-topbar">
      <span className="topbar__brand">scad studio</span>
      <span className="topbar__sep" aria-hidden="true" />
      <nav className="topbar__crumb" aria-label="workspace breadcrumb">
        <span>workspace</span>
        <span className="topbar__slash">/</span>
        <b data-testid="workspace-name">{workspaceName}</b>
      </nav>
      <span className="topbar__spacer" />
      <span
        className="topbar__meta"
        data-testid="connection-status"
        title={message || undefined}
      >
        <span className={statusDotClass(status)} aria-hidden="true" />
        <span>{statusText(status)}</span>
      </span>
      <span className="topbar__meta" data-testid="ws-url" title={wsUrl}>
        <span>{wsUrl}</span>
      </span>
    </header>
  );
}

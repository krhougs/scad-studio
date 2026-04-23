// Topbar — Buddin titleblock. Logo / 分隔 / 面包屑 / spacer / 连接状态 / ws URL。

import { Users } from "lucide-react";

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

function dotClass(status: TopbarStatus): string {
  if (status === "ready") return "dot";
  if (status === "busy") return "dot dot--live";
  if (status === "error") return "dot dot--err";
  if (status === "idle") return "dot dot--off";
  return "dot dot--off";
}

export function Topbar({ workspaceName, wsUrl, status, message }: TopbarProps) {
  const host = wsUrl.replace(/^wss?:\/\//, "");
  return (
    <div className="topbar" data-testid="workbench-topbar">
      <a className="logo" href="/" aria-label="scad studio home">
        scad studio
      </a>
      <div className="sep-v" aria-hidden="true" />
      <div className="crumb" aria-label="workspace breadcrumb">
        <span>workspace</span>
        <span className="sl">/</span>
        <b data-testid="workspace-name">{workspaceName}</b>
      </div>
      <div className="spacer" />
      <div
        className="meta"
        data-testid="connection-status"
        title={message || undefined}
      >
        <span className={dotClass(status)} aria-hidden="true" />
        <span>{statusText(status)}</span>
      </div>
      <div className="meta" data-testid="ws-url" title={wsUrl}>
        <Users size={12} strokeWidth={1.5} aria-hidden="true" />
        <span>{host}</span>
      </div>
    </div>
  );
}

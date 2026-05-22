import type { ReactNode } from "react";

type SidePanelHeaderProps = {
  title: string;
  meta?: string;
  actions?: ReactNode;
};

export function SidePanelHeader({ title, meta, actions }: SidePanelHeaderProps) {
  return (
    <header className="side-panel__head">
      <div className="side-panel__title-group">
        <div className="title">§ {title}</div>
        {meta ? <div className="sub">{meta}</div> : null}
      </div>
      {actions ? <div className="side-panel__actions">{actions}</div> : null}
    </header>
  );
}

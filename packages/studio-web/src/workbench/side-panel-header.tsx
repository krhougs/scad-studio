type SidePanelHeaderProps = {
  title: string;
  meta?: string;
};

export function SidePanelHeader({ title, meta }: SidePanelHeaderProps) {
  return (
    <header className="side-panel__head">
      <div>
        <div className="title">§ {title}</div>
        {meta ? <div className="sub">{meta}</div> : null}
      </div>
    </header>
  );
}

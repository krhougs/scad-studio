// Rail: icon-only navigation. Phase 4 只保留壳层与 aria-label；具体跳转行为
// 在 Phase 5+ 接入。无 emoji，图标用简单 SVG glyph，保持 datasheet 风格。

import { useUiStore } from "../state/ui-store";

type RailItem = {
  id: string;
  label: string;
  shape: "workspace" | "chat" | "settings";
};

const ITEMS: RailItem[] = [
  { id: "workspace", label: "workspace", shape: "workspace" },
  { id: "chat", label: "agent chat", shape: "chat" },
];

const FOOTER_ITEMS: RailItem[] = [
  { id: "settings", label: "settings", shape: "settings" },
];

function Glyph({ shape }: { shape: RailItem["shape"] }) {
  switch (shape) {
    case "workspace":
      return (
        <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
          <rect x="1" y="3" width="14" height="10" fill="none" stroke="currentColor" strokeWidth="1" />
          <path d="M1 6h14" stroke="currentColor" strokeWidth="1" />
        </svg>
      );
    case "chat":
      return (
        <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
          <path
            d="M2 3h12v8H6l-3 3V3z"
            fill="none"
            stroke="currentColor"
            strokeWidth="1"
          />
        </svg>
      );
    case "settings":
      return (
        <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
          <circle cx="8" cy="8" r="2.5" fill="none" stroke="currentColor" strokeWidth="1" />
          <path
            d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.3 3.3l1.4 1.4M11.3 11.3l1.4 1.4M3.3 12.7l1.4-1.4M11.3 4.7l1.4-1.4"
            stroke="currentColor"
            strokeWidth="1"
          />
        </svg>
      );
  }
}

function RailButton({ item, active }: { item: RailItem; active: boolean }) {
  const setActiveRail = useUiStore((s) => s.setActiveRail);
  return (
    <button
      type="button"
      className={`rail__button${active ? " is-active" : ""}`}
      aria-label={item.label}
      title={item.label}
      aria-current={active ? "page" : undefined}
      onClick={() => setActiveRail(item.id)}
      data-testid={`rail-${item.id}`}
    >
      <Glyph shape={item.shape} />
    </button>
  );
}

export function Rail() {
  const activeRail = useUiStore((s) => s.activeRail);
  return (
    <nav className="workbench__rail" aria-label="primary">
      {ITEMS.map((item) => (
        <RailButton key={item.id} item={item} active={activeRail === item.id} />
      ))}
      <div className="rail__separator" aria-hidden="true" />
      <div className="rail__grow" />
      {FOOTER_ITEMS.map((item) => (
        <RailButton key={item.id} item={item} active={activeRail === item.id} />
      ))}
    </nav>
  );
}

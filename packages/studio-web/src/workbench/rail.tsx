// Rail — 52px 导航条，lucide 图标。点击切换 Zustand activeRail。

import {
  Box,
  FolderOpen,
  History,
  Layers,
  MessageSquare,
  Printer,
  Settings2,
} from "lucide-react";
import type { ComponentType, SVGProps } from "react";
import { useUiStore } from "../state/ui-store";

type RailItem = {
  id: string;
  label: string;
  Icon: ComponentType<SVGProps<SVGSVGElement> & { size?: number | string }>;
};

const ITEMS: RailItem[] = [
  { id: "chat", label: "agent", Icon: MessageSquare },
  { id: "workspace", label: "library", Icon: FolderOpen },
  { id: "parts", label: "parts", Icon: Box },
  { id: "materials", label: "materials", Icon: Layers },
];

const FOOTER_TOP: RailItem[] = [
  { id: "queue", label: "print queue", Icon: Printer },
  { id: "history", label: "history", Icon: History },
];

const FOOTER_BOTTOM: RailItem[] = [
  { id: "settings", label: "settings", Icon: Settings2 },
];

function RailButton({ item, active }: { item: RailItem; active: boolean }) {
  const setActiveRail = useUiStore((s) => s.setActiveRail);
  const { Icon } = item;
  return (
    <button
      type="button"
      className={active ? "active" : undefined}
      aria-label={item.label}
      title={item.label}
      aria-current={active ? "page" : undefined}
      onClick={() => setActiveRail(item.id)}
      data-testid={`rail-${item.id}`}
    >
      <Icon size={18} strokeWidth={1.5} aria-hidden="true" />
    </button>
  );
}

export function Rail() {
  const activeRail = useUiStore((s) => s.activeRail);
  return (
    <nav className="rail" aria-label="primary">
      {ITEMS.map((item) => (
        <RailButton key={item.id} item={item} active={activeRail === item.id} />
      ))}
      <div className="sep" aria-hidden="true" />
      {FOOTER_TOP.map((item) => (
        <RailButton key={item.id} item={item} active={activeRail === item.id} />
      ))}
      <div className="grow" />
      {FOOTER_BOTTOM.map((item) => (
        <RailButton key={item.id} item={item} active={activeRail === item.id} />
      ))}
    </nav>
  );
}

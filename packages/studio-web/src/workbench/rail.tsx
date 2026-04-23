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
import { useLocation, useNavigate } from "react-router-dom";
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

type RailButtonProps = {
  item: RailItem;
  active: boolean;
  onClick: () => void;
};

function RailButton({ item, active, onClick }: RailButtonProps) {
  const { Icon } = item;
  return (
    <button
      type="button"
      className={active ? "active" : undefined}
      aria-label={item.label}
      title={item.label}
      aria-current={active ? "page" : undefined}
      onClick={onClick}
      data-testid={`rail-${item.id}`}
    >
      <Icon size={18} strokeWidth={1.5} aria-hidden="true" />
    </button>
  );
}

export function Rail() {
  const activeRail = useUiStore((s) => s.activeRail);
  const setActiveRail = useUiStore((s) => s.setActiveRail);
  const navigate = useNavigate();
  const location = useLocation();

  const handleClick = (id: string) => {
    setActiveRail(id);
    if (id === "settings") {
      navigate("/settings");
    } else if (location.pathname !== "/") {
      navigate("/");
    }
  };

  const isActive = (id: string): boolean => {
    if (id === "settings") return location.pathname === "/settings";
    return activeRail === id && location.pathname === "/";
  };

  return (
    <nav className="rail" aria-label="primary">
      {ITEMS.map((item) => (
        <RailButton
          key={item.id}
          item={item}
          active={isActive(item.id)}
          onClick={() => handleClick(item.id)}
        />
      ))}
      <div className="sep" aria-hidden="true" />
      {FOOTER_TOP.map((item) => (
        <RailButton
          key={item.id}
          item={item}
          active={isActive(item.id)}
          onClick={() => handleClick(item.id)}
        />
      ))}
      <div className="grow" />
      {FOOTER_BOTTOM.map((item) => (
        <RailButton
          key={item.id}
          item={item}
          active={isActive(item.id)}
          onClick={() => handleClick(item.id)}
        />
      ))}
    </nav>
  );
}

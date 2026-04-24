import {
  ChatTeardropText,
  ClockCounterClockwise,
  Cube,
  FolderOpen,
  GearSix,
  Printer,
  Stack,
  TerminalWindow,
  type Icon,
} from "@phosphor-icons/react";
import { useSearchParams } from "react-router-dom";
import {
  type LeftPanelId,
  LEFT_PANEL_PARAM,
  normalizeLeftPanelId,
} from "./left-panel-routing";

type RailItem = {
  id: LeftPanelId;
  label: string;
  Icon: Icon;
};

const ITEMS: RailItem[] = [
  { id: "chat", label: "agent", Icon: ChatTeardropText },
  { id: "files", label: "files", Icon: FolderOpen },
  { id: "parts", label: "parts", Icon: Cube },
  { id: "materials", label: "materials", Icon: Stack },
];

const FOOTER_TOP: RailItem[] = [
  { id: "queue", label: "print queue", Icon: Printer },
  { id: "history", label: "history", Icon: ClockCounterClockwise },
];

const FOOTER_BOTTOM: RailItem[] = [
  { id: "log", label: "log", Icon: TerminalWindow },
  { id: "settings", label: "settings", Icon: GearSix },
];

type RailButtonProps = {
  item: RailItem;
  active: boolean;
  onClick: () => void;
};

function RailButton({ item, active, onClick }: RailButtonProps) {
  const { Icon: IconComponent } = item;
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
      <IconComponent size={18} weight="bold" aria-hidden="true" />
    </button>
  );
}

export function Rail() {
  const [searchParams, setSearchParams] = useSearchParams();
  const activeRail = normalizeLeftPanelId(searchParams.get(LEFT_PANEL_PARAM));

  const handleClick = (id: LeftPanelId) => {
    setSearchParams((prev) => {
      prev.set(LEFT_PANEL_PARAM, id);
      return prev;
    });
  };

  const renderItem = (item: RailItem) => (
    <RailButton
      key={item.id}
      item={item}
      active={activeRail === item.id}
      onClick={() => handleClick(item.id)}
    />
  );

  return (
    <nav className="rail" aria-label="primary">
      {ITEMS.map(renderItem)}
      <div className="sep" aria-hidden="true" />
      {FOOTER_TOP.map(renderItem)}
      <div className="grow" />
      {FOOTER_BOTTOM.map(renderItem)}
    </nav>
  );
}

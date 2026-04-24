// Tab Bar: renders openTabs / activeTabId from the UI store. Close button is
// a literal × glyph per the Buddin datasheet. The active tab draws a 1px
// bottom border to match §九 (buttons & tabs) of the design system.

import type { DocumentTab } from "../state/ui-store";
import type React from "react";

type TabBarProps = {
  tabs: DocumentTab[];
  activeTabId: string | null;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
};

export function TabBar({ tabs, activeTabId, onActivate, onClose }: TabBarProps) {
  const handleWheel = (event: React.WheelEvent<HTMLDivElement>) => {
    const el = event.currentTarget;
    if (el.scrollWidth <= el.clientWidth) return;
    event.preventDefault();
    el.scrollLeft += event.deltaY + event.deltaX;
  };

  if (tabs.length === 0) {
    return (
      <div className="tabbar tabbar--empty" data-testid="tabbar">
        <span className="tabbar__empty-label">no document open</span>
      </div>
    );
  }
  return (
    <div
      className="tabbar"
      role="tablist"
      data-testid="tabbar"
      onWheel={handleWheel}
    >
      {tabs.map((tab) => {
        const isActive = tab.id === activeTabId;
        return (
          <div
            key={tab.id}
            role="tab"
            aria-selected={isActive}
            className={`tabbar__tab${isActive ? " is-active" : ""}`}
            data-testid={`tab-${tab.label}`}
          >
            <button
              type="button"
              className="tabbar__label"
              onClick={() => onActivate(tab.id)}
            >
              {tab.label}
            </button>
            <button
              type="button"
              className="tabbar__close"
              aria-label={`close ${tab.label}`}
              data-testid={`tab-close-${tab.label}`}
              onClick={(event) => {
                event.stopPropagation();
                onClose(tab.id);
              }}
            >
              ×
            </button>
          </div>
        );
      })}
    </div>
  );
}

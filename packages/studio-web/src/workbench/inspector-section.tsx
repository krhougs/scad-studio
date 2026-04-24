import { useState } from "react";
import type React from "react";

type InspectorSectionProps = {
  id: string;
  title: string;
  defaultOpen?: boolean;
  actions?: React.ReactNode;
  children: React.ReactNode;
};

export function InspectorSection({
  id,
  title,
  defaultOpen = true,
  actions,
  children,
}: InspectorSectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section className="insp-sec" data-testid={`inspector-section-${id}`}>
      <h5>
        <button
          type="button"
          className="insp-sec__toggle"
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
          data-testid={`inspector-section-${id}-toggle`}
        >
          <span>{title}</span>
          <span className="insp-sec__marker" aria-hidden="true">
            {open ? "-" : "+"}
          </span>
        </button>
        {actions}
      </h5>
      <div
        className="insp-sec__body"
        hidden={!open}
        data-testid={`inspector-section-${id}-body`}
      >
        {children}
      </div>
    </section>
  );
}

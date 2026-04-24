import { CaretDown, CaretRight } from "@phosphor-icons/react";
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
  const Icon = open ? CaretDown : CaretRight;
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
          <Icon size={12} weight="bold" aria-hidden="true" />
          <span>{title}</span>
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

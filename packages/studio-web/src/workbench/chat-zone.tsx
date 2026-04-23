// Chat zone —— Buddin agent 会话壳层。Phase 4 维持占位文案，不模拟假消息；
// agent 真实接入留给后续 phase。

import { ArrowUp, Box, Paperclip, Ruler } from "lucide-react";
import { useUiStore } from "../state/ui-store";

export function ChatZone() {
  const inputDraft = useUiStore((s) => s.inputDraft);
  const setInputDraft = useUiStore((s) => s.setInputDraft);

  return (
    <section className="chat" data-testid="workbench-chat" aria-label="agent">
      <header className="chat-head">
        <div>
          <div className="title">§ agent</div>
          <div className="sub">session placeholder</div>
        </div>
        <button type="button" className="btn btn--ghost btn--sm" disabled>
          new
        </button>
      </header>
      <div className="chat-body" data-testid="chat-body">
        <p className="chat-empty">
          agent session not connected yet. subsequent phases will render
          structured agent replies here.
        </p>
      </div>
      <footer className="chat-input">
        <div className="wrap">
          <textarea
            placeholder="Describe a change, add a feature, or ask for alternatives..."
            value={inputDraft}
            onChange={(ev) => setInputDraft(ev.target.value)}
            data-testid="chat-input"
          />
          <div className="tools">
            <div className="tools-left">
              <button type="button" title="attach sketch" disabled>
                <Paperclip size={14} strokeWidth={1.5} aria-hidden="true" />
              </button>
              <button type="button" title="reference part" disabled>
                <Box size={14} strokeWidth={1.5} aria-hidden="true" />
              </button>
              <button type="button" title="dimension pick" disabled>
                <Ruler size={14} strokeWidth={1.5} aria-hidden="true" />
              </button>
            </div>
            <button type="button" className="send" disabled>
              send <ArrowUp size={12} strokeWidth={2} aria-hidden="true" />
            </button>
          </div>
        </div>
      </footer>
    </section>
  );
}

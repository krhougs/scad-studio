// Chat zone: Phase 4 只做布局壳层，chat 的业务接入（agent 真实会话）留到
// 后续 Phase。输入草稿仍走 Zustand ui-store，保持 UI 壳状态归属清晰。

import { useUiStore } from "../state/ui-store";

export function ChatZone() {
  const inputDraft = useUiStore((s) => s.inputDraft);
  const setInputDraft = useUiStore((s) => s.setInputDraft);

  return (
    <section
      className="workbench__chat"
      data-testid="workbench-chat"
      aria-label="agent chat"
    >
      <header className="chat__head">
        <div>
          <div className="chat__kicker">§ agent</div>
          <div className="chat__sub">session placeholder</div>
        </div>
        <button type="button" className="btn btn--ghost btn--sm" disabled>
          new
        </button>
      </header>
      <div className="chat__body" data-testid="chat-body">
        <p className="chat__placeholder">
          agent 会话尚未接入。后续 phase 将把 agent 回执（具体数值的 mono 卡片）
          渲染在此区域。
        </p>
      </div>
      <footer className="chat__input">
        <div className="chat__wrap">
          <textarea
            className="chat__textarea"
            placeholder="Describe a change, add a feature, or ask for alternatives..."
            value={inputDraft}
            onChange={(ev) => setInputDraft(ev.target.value)}
            data-testid="chat-input"
          />
          <div className="chat__tools">
            <button type="button" className="btn btn--solid btn--sm" disabled>
              send
            </button>
          </div>
        </div>
      </footer>
    </section>
  );
}

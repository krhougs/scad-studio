import { ArrowUp, Cube, Paperclip, Ruler, X } from "@phosphor-icons/react";
import type { AgentOperationLevel } from "@budn/app-server-protocol";
import type { ContextPill } from "./chat-zone";

export function ChatComposer(props: {
  value: string;
  disabled: boolean;
  operation: AgentOperationLevel;
  contextPills: ContextPill[];
  onChange: (value: string) => void;
  onOperationChange: (value: AgentOperationLevel) => void;
  onRemovePill: (refText: string) => void;
  onSend: () => void;
}) {
  return (
    <footer className="chat-input">
      <div className="wrap">
        <ContextPillBar pills={props.contextPills} onRemove={props.onRemovePill} />
        <ChatTextarea
          value={props.value}
          onChange={props.onChange}
          onSend={props.onSend}
        />
        <ChatComposerTools
          disabled={props.disabled}
          operation={props.operation}
          onOperationChange={props.onOperationChange}
          onSend={props.onSend}
        />
      </div>
    </footer>
  );
}

function ContextPillBar(props: {
  pills: ContextPill[];
  onRemove: (refText: string) => void;
}) {
  if (props.pills.length === 0) return null;
  return (
    <div className="context-pill-bar" data-testid="context-pill-bar">
      {props.pills.map((pill) => (
        <span key={pill.ref_text} className="context-pill">
          <code>{pill.display}</code>
          <button
            type="button"
            className="pill-remove"
            aria-label={`remove ${pill.display}`}
            onClick={() => props.onRemove(pill.ref_text)}
          >
            <X size={10} weight="bold" />
          </button>
        </span>
      ))}
    </div>
  );
}

function ChatTextarea(props: {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
}) {
  return (
    <textarea
      placeholder="Describe what you want to build or change..."
      value={props.value}
      onChange={(ev) => props.onChange((ev.target as HTMLTextAreaElement).value)}
      onKeyDown={(ev) => {
        if (ev.key === "Enter" && (ev.metaKey || ev.ctrlKey)) props.onSend();
      }}
      data-testid="chat-input"
    />
  );
}

function ChatComposerTools(props: {
  disabled: boolean;
  operation: AgentOperationLevel;
  onOperationChange: (value: AgentOperationLevel) => void;
  onSend: () => void;
}) {
  return (
    <div className="tools">
      <div className="tools-left">
        <OperationSelect
          disabled={props.disabled}
          value={props.operation}
          onChange={props.onOperationChange}
        />
        <DisabledToolButtons />
      </div>
      <button
        type="button"
        className="send"
        disabled={props.disabled}
        onClick={props.onSend}
      >
        send <ArrowUp size={12} weight="bold" aria-hidden="true" />
      </button>
    </div>
  );
}

function OperationSelect(props: {
  disabled: boolean;
  value: AgentOperationLevel;
  onChange: (value: AgentOperationLevel) => void;
}) {
  return (
    <select
      aria-label="agent operation"
      className="operation-select"
      disabled={props.disabled}
      value={props.value}
      onChange={(event) =>
        props.onChange((event.target as HTMLSelectElement).value as AgentOperationLevel)
      }
    >
      <option value="auto">auto</option>
      <option value="inform">inform</option>
      <option value="plan">plan</option>
      <option value="execute">execute</option>
    </select>
  );
}

function DisabledToolButtons() {
  return (
    <>
      <button type="button" title="attach sketch" disabled>
        <Paperclip size={14} weight="bold" aria-hidden="true" />
      </button>
      <button type="button" title="reference part" disabled>
        <Cube size={14} weight="bold" aria-hidden="true" />
      </button>
      <button type="button" title="dimension pick" disabled>
        <Ruler size={14} weight="bold" aria-hidden="true" />
      </button>
    </>
  );
}

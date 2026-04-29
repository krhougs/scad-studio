import { ArrowUp, Cube, Paperclip, Ruler, X } from "@phosphor-icons/react";
import type { AgentMode } from "@budn/app-server-protocol";
import { ComposerPrimitive } from "@assistant-ui/react";
import type { ContextPill } from "./chat-zone";

export function ChatComposer(props: {
  disabled: boolean;
  mode: AgentMode;
  contextPills: ContextPill[];
  onModeChange: (value: AgentMode) => void;
  onRemovePill: (refText: string) => void;
}) {
  return (
    <ComposerPrimitive.Root className="chat-input">
      <div className="wrap">
        <ContextPillBar pills={props.contextPills} onRemove={props.onRemovePill} />
        <ComposerPrimitive.Input
          placeholder="Describe what you want to build or change..."
          submitMode="ctrlEnter"
          data-testid="chat-input"
          aria-label="chat message"
        />
        <ChatComposerTools
          disabled={props.disabled}
          mode={props.mode}
          onModeChange={props.onModeChange}
        />
      </div>
    </ComposerPrimitive.Root>
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

function ChatComposerTools(props: {
  disabled: boolean;
  mode: AgentMode;
  onModeChange: (value: AgentMode) => void;
}) {
  return (
    <div className="tools">
      <div className="tools-left">
        <OperationSelect
          disabled={props.disabled}
          value={props.mode}
          onChange={props.onModeChange}
        />
        <DisabledToolButtons />
      </div>
      <ComposerPrimitive.Send className="send">
        send <ArrowUp size={12} weight="bold" aria-hidden="true" />
      </ComposerPrimitive.Send>
    </div>
  );
}

function OperationSelect(props: {
  disabled: boolean;
  value: AgentMode;
  onChange: (value: AgentMode) => void;
}) {
  return (
    <select
      aria-label="agent mode"
      className="operation-select"
      disabled={props.disabled}
      value={props.value}
      onChange={(event) =>
        props.onChange((event.target as HTMLSelectElement).value as AgentMode)
      }
    >
      <option value="agent">Agent</option>
      <option value="plan">Plan</option>
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

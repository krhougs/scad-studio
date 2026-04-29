import MarkdownPreview from "@uiw/react-markdown-preview";
import rehypeSanitize from "rehype-sanitize";
import type { AgentErrorType } from "@budn/app-server-protocol";
import { markdownSanitizeSchema } from "../viewers/markdown-security";
import type { ChatMessageRecord, AgentEvent } from "./chat-zone";

const mdWrapperElement = { "data-color-mode": "dark" } as const;

export function ChatBody(props: {
  messages: ChatMessageRecord[];
  agentEvents: AgentEvent[];
  llmConfigured: boolean;
  streaming: boolean;
  streamText: string;
}) {
  if (
    props.messages.length === 0 &&
    props.agentEvents.length === 0 &&
    !props.streaming
  ) {
    if (!props.llmConfigured) return <LlmSetupGuide />;
    return <WelcomeEmptyState />;
  }
  return (
    <div className="chat-body" data-testid="chat-body">
      {props.messages.map((message) => (
        <ChatMessage key={message.message_id} message={message} />
      ))}
      {props.agentEvents.map((event, index) => (
        <AgentEventRow key={`${event.event}-${index}`} event={event} />
      ))}
      {props.streamText && <StreamingMessage text={props.streamText} />}
      {!props.streamText && props.streaming && <ThinkingIndicator />}
    </div>
  );
}

function StreamingMessage({ text }: { text: string }) {
  return (
    <article className="msg agent">
      <div className="who"><b>assistant</b></div>
      <div className="bubble">
        <MarkdownPreview
          source={text}
          wrapperElement={mdWrapperElement}
          className="chat-markdown"
          rehypePlugins={[[rehypeSanitize, markdownSanitizeSchema]]}
        />
      </div>
    </article>
  );
}

function ThinkingIndicator() {
  return (
    <div className="agent-thinking" data-testid="agent-thinking">
      <span className="thinking-dot" />
      <span className="thinking-dot" />
      <span className="thinking-dot" />
    </div>
  );
}

function LlmSetupGuide() {
  return (
    <div className="chat-body chat-empty-state" data-testid="llm-setup-guide">
      <div className="welcome-title">AI service not configured</div>
      <p className="welcome-desc">
        Set the following environment variables to enable the agent:
      </p>
      <ul className="welcome-suggestions">
        <li><code>BUDN_LLM_BASE_URL</code></li>
        <li><code>BUDN_LLM_API_KEY</code></li>
        <li><code>BUDN_LLM_MODEL</code> (optional, defaults to gpt-4o)</li>
      </ul>
      <p className="welcome-hint">
        Restart the server after setting the variables.
      </p>
    </div>
  );
}

function WelcomeEmptyState() {
  return (
    <div className="chat-body chat-empty-state" data-testid="chat-empty-state">
      <div className="welcome-title">budn&apos; agent</div>
      <p className="welcome-desc">
        Describe what you want to build or modify.
      </p>
      <ul className="welcome-suggestions">
        <li>Design a phone case</li>
        <li>Modify the lid height</li>
        <li>Explain how CadQuery fillet works</li>
      </ul>
      <p className="welcome-hint">
        Select parts in the Viewer to add context to your message.
      </p>
    </div>
  );
}

function ChatMessage({ message }: { message: ChatMessageRecord }) {
  if (message.role === "meta") return null;
  const role = message.role === "user" ? "user" : "agent";
  return (
    <article className={`msg ${role}`}>
      <div className="who">
        <b>{message.role}</b>
        <time>{formatTime(message.ts_ms)}</time>
      </div>
      <div className="bubble">
        {role === "agent" ? (
          <MarkdownPreview
            source={message.content}
            wrapperElement={mdWrapperElement}
            className="chat-markdown"
            rehypePlugins={[[rehypeSanitize, markdownSanitizeSchema]]}
          />
        ) : (
          message.content
        )}
      </div>
    </article>
  );
}

export function AgentEventRow({ event }: { event: AgentEvent }) {
  if (event.event === "agent.error") {
    return <AgentErrorCard event={event} />;
  }
  const label = event.event.replace("agent.", "");
  const detail = agentEventDetail(event);
  return (
    <div className="agent-op">
      <div className="op-head">
        <span className={event.event === "agent.done" ? "ok" : undefined}>
          {label}
        </span>
      </div>
      <div className="op-detail">{detail}</div>
    </div>
  );
}

function AgentErrorCard({ event }: { event: AgentEvent }) {
  const payload = event.payload ?? {};
  const errorType = stringField(payload, "error_type") as AgentErrorType;
  const message = stringField(payload, "message");
  const friendly = friendlyErrorMessage(errorType);
  return (
    <div className="agent-error-card" data-testid="agent-error-card">
      <div className="agent-error-header">{friendly.title}</div>
      <p className="agent-error-desc">{friendly.hint}</p>
      {message && <pre className="agent-error-detail">{message}</pre>}
    </div>
  );
}

function agentEventDetail(event: AgentEvent): string {
  const payload = event.payload ?? {};
  if (event.event === "agent.error") return stringField(payload, "message");
  if (event.event === "agent.tool_start")
    return stringField(payload, "tool_name");
  if (event.event === "agent.tool_result")
    return stringField(payload, "result_json");
  if (event.event === "agent.mesh_ready") return "mesh ready";
  if (event.event === "agent.done") {
    return payload["cancelled"] === true ? "cancelled" : "done";
  }
  if (event.event === "agent.plan_proposed") {
    return stringField(payload, "change_description") || "plan proposed";
  }
  return event.event;
}

export function findAffectedAssemblies(paths: string[]): string[] {
  return paths.filter((p) => p.startsWith("assemblies/") || p.includes("/assemblies/"));
}

type FriendlyError = { title: string; hint: string };

const ERROR_MESSAGES: Record<string, FriendlyError> = {
  llm_error: {
    title: "AI service error",
    hint: "The AI service returned an error. Check your LLM configuration and try again.",
  },
  llm_refused: {
    title: "Request refused",
    hint: "The AI service refused the request. Try rephrasing your prompt.",
  },
  cadquery_build_error: {
    title: "CadQuery build failed",
    hint: "The generated CadQuery code failed to build. The agent will try to fix it.",
  },
  python_import_error: {
    title: "Python import error",
    hint: "A required Python module could not be imported. Check your CadQuery installation.",
  },
  tessellation_error: {
    title: "Mesh generation failed",
    hint: "Could not generate a mesh from the model. The geometry may be invalid.",
  },
  topology_mapping_error: {
    title: "Topology mapping failed",
    hint: "Could not map topology data. Try regenerating the model.",
  },
  export_error: {
    title: "Export failed",
    hint: "Could not export the model to the requested format.",
  },
  timeout: {
    title: "Operation timed out",
    hint: "The operation took too long. Try simplifying the request.",
  },
  file_conflict: {
    title: "File conflict",
    hint: "The target file was modified externally. Refresh and try again.",
  },
  permission_denied: {
    title: "Permission denied",
    hint: "The operation was not permitted. Switch to Agent mode or run an existing plan.",
  },
};

export function friendlyErrorMessage(errorType: string): FriendlyError {
  return ERROR_MESSAGES[errorType] ?? { title: "Error", hint: "An unexpected error occurred." };
}

function stringField(payload: Record<string, unknown>, key: string): string {
  const value = payload[key];
  return typeof value === "string" ? value : "";
}

function formatTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "";
  return new Date(value).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

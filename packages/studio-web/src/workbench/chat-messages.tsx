import { useEffect, useMemo, useRef } from "react";
import MarkdownPreview from "@uiw/react-markdown-preview";
import rehypeSanitize from "rehype-sanitize";
import type { AgentErrorType } from "@budn/app-server-protocol";
import { markdownSanitizeSchema } from "../viewers/markdown-security";
import type { ChatMessageRecord, AgentEvent } from "./chat-zone";
import { pathSegments } from "./path-utils";

const mdWrapperElement = { "data-color-mode": "dark" } as const;

export function ChatBody(props: {
  messages: ChatMessageRecord[];
  agentEvents: AgentEvent[];
  llmConfigured: boolean;
  streaming: boolean;
  planActionDisabled: boolean;
  onOpenPlan?: (path: unknown) => void;
  onRunPlan?: (plan: PlanRunAction) => void;
}) {
  const visibleRunId = useMemo(
    () => latestAgentRunId(props.agentEvents),
    [props.agentEvents],
  );
  const assistantMessageCount = useMemo(
    () => countMessagesByRole(props.messages, "assistant"),
    [props.messages],
  );
  const runHistoryBaselineRef = useRef<RunHistoryBaseline | null>(null);
  const runHistoryBaseline = runHistoryBaselineRef.current;
  const historyCoversLiveRun = Boolean(
    visibleRunId &&
    !props.streaming &&
    runHistoryBaseline?.runId === visibleRunId &&
    props.messages.length > runHistoryBaseline.messageCount &&
    assistantMessageCount > runHistoryBaseline.assistantCount,
  );
  const timeline = useMemo(
    () => buildAgentTimeline(props.agentEvents, props.messages, historyCoversLiveRun),
    [props.agentEvents, props.messages, historyCoversLiveRun],
  );
  const timelineKey = timeline.map(timelineItemKey).join("|");
  const scrollAnchorRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!visibleRunId) {
      runHistoryBaselineRef.current = null;
      return;
    }
    if (
      props.streaming &&
      runHistoryBaselineRef.current?.runId !== visibleRunId
    ) {
      runHistoryBaselineRef.current = {
        runId: visibleRunId,
        messageCount: props.messages.length,
        assistantCount: assistantMessageCount,
      };
    }
  }, [visibleRunId, props.streaming, props.messages.length, assistantMessageCount]);
  useEffect(() => {
    const scrollIntoView = scrollAnchorRef.current?.scrollIntoView;
    if (typeof scrollIntoView === "function") {
      scrollIntoView.call(scrollAnchorRef.current, { block: "end" });
    }
  }, [props.messages.length, props.streaming, timelineKey]);

  if (
    props.messages.length === 0 &&
    timeline.length === 0 &&
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
      {timeline.map((item) => (
        item.kind === "stream" ? (
          <StreamingMessage key={item.key} text={item.text} />
        ) : (
          <AgentEventRow
            key={item.key}
            event={item.event}
            planActionDisabled={props.planActionDisabled}
            onOpenPlan={props.onOpenPlan}
            onRunPlan={props.onRunPlan}
          />
        )
      ))}
      {props.streaming && <ThinkingIndicator />}
      <div ref={scrollAnchorRef} data-testid="chat-scroll-anchor" />
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

export type PlanRunAction = {
  planId: string;
  planRef: unknown;
};

export function AgentEventRow({
  event,
  planActionDisabled = false,
  onOpenPlan,
  onRunPlan,
}: {
  event: AgentEvent;
  planActionDisabled?: boolean;
  onOpenPlan?: (path: unknown) => void;
  onRunPlan?: (plan: PlanRunAction) => void;
}) {
  if (event.event === "agent.error") {
    return <AgentErrorCard event={event} />;
  }
  if (event.event === "agent.plan_saved") {
    const plan = parsePlanSavedEvent(event);
    if (plan) {
      return (
        <PlanPackageCard
          plan={plan}
          actionDisabled={planActionDisabled}
          onOpenPlan={onOpenPlan}
          onRunPlan={onRunPlan}
        />
      );
    }
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

type PlanPackageCardData = {
  planId: string;
  status: string;
  targetPath: string;
  affectedFiles: string[];
  newFiles: string[];
  exportTargets: string[];
  planRef: unknown;
  planPath: unknown;
};

function PlanPackageCard(props: {
  plan: PlanPackageCardData;
  actionDisabled: boolean;
  onOpenPlan?: (path: unknown) => void;
  onRunPlan?: (plan: PlanRunAction) => void;
}) {
  const { plan } = props;
  return (
    <div className="agent-op plan-package-card" data-testid="plan-package-card">
      <div className="op-head">
        <span>{plan.status || "planned"}</span>
      </div>
      <div className="plan-card-title">{plan.planId}</div>
      <PlanField label="target" values={[plan.targetPath]} />
      <PlanField label="affected" values={plan.affectedFiles} />
      <PlanField label="new" values={plan.newFiles} />
      <PlanField label="exports" values={plan.exportTargets} />
      <div className="plan-card-actions">
        <button type="button" onClick={() => props.onOpenPlan?.(plan.planPath)}>
          Open Plan
        </button>
        <button
          type="button"
          disabled={props.actionDisabled}
          onClick={() =>
            props.onRunPlan?.({ planId: plan.planId, planRef: plan.planRef })
          }
        >
          Run Plan
        </button>
      </div>
    </div>
  );
}

function PlanField(props: { label: string; values: string[] }) {
  const values = props.values.length > 0 ? props.values : ["none"];
  return (
    <div className="plan-card-field">
      <span>{props.label}</span>
      <code>{values.join(", ")}</code>
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

function parsePlanSavedEvent(event: AgentEvent): PlanPackageCardData | null {
  const payload = event.payload ?? {};
  const planPackage = objectField(payload, "package");
  const planId = stringField(planPackage, "plan_id");
  const planRef = planPackage["plan_ref"];
  const planPath = planPackage["plan_path"];
  if (!planId || !planRef || !planPath) return null;
  if (!isPlanMarkdownPath(planId, planRef, planPath)) return null;
  return {
    planId,
    status: stringField(payload, "status"),
    targetPath: pathText(payload["target_path"]),
    affectedFiles: pathList(payload["affected_files"]),
    newFiles: pathList(payload["new_files"]),
    exportTargets: pathList(payload["export_targets"]),
    planRef,
    planPath,
  };
}

export function findAffectedAssemblies(paths: string[]): string[] {
  return paths.filter((p) => p.startsWith("assemblies/") || p.includes("/assemblies/"));
}

type TimelineItem =
  | { kind: "stream"; key: string; text: string }
  | { kind: "event"; key: string; event: AgentEvent };

type RunHistoryBaseline = {
  runId: string;
  messageCount: number;
  assistantCount: number;
};

function buildAgentTimeline(
  events: AgentEvent[],
  messages: ChatMessageRecord[],
  historyCoversLiveRun: boolean,
): TimelineItem[] {
  const fullTokenText = events.map(tokenText).join("");
  const coveredAssistant = latestAssistantMessage(messages);
  const hideTokenSegments =
    historyCoversLiveRun &&
    fullTokenText.length > 0 &&
    Boolean(coveredAssistant?.content.includes(fullTokenText));
  const timeline: TimelineItem[] = [];
  let pendingText = "";
  let streamIndex = 0;

  const flushText = () => {
    if (!pendingText) return;
    if (!hideTokenSegments) {
      timeline.push({
        kind: "stream",
        key: `stream-${streamIndex}`,
        text: pendingText,
      });
      streamIndex += 1;
    }
    pendingText = "";
  };

  events.forEach((event, index) => {
    const text = tokenText(event);
    if (text) {
      pendingText += text;
      return;
    }
    flushText();
    timeline.push({ kind: "event", key: `event-${index}-${event.event}`, event });
  });
  flushText();

  return timeline;
}

function latestAssistantMessage(messages: ChatMessageRecord[]): ChatMessageRecord | null {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index] as ChatMessageRecord | undefined;
    if (message?.role === "assistant") return message;
  }
  return null;
}

function countMessagesByRole(
  messages: ChatMessageRecord[],
  role: ChatMessageRecord["role"],
): number {
  return messages.filter((message) => message.role === role).length;
}

function latestAgentRunId(events: AgentEvent[]): string | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const runId = agentEventRunId(events[index] as AgentEvent | undefined);
    if (runId) return runId;
  }
  return null;
}

function agentEventRunId(event: AgentEvent | undefined): string | null {
  const runId = event?.payload?.["run_id"];
  return typeof runId === "string" ? runId : null;
}

function timelineItemKey(item: TimelineItem): string {
  if (item.kind === "stream") return `${item.key}:${item.text}`;
  return `${item.key}:${JSON.stringify(item.event.payload ?? {})}`;
}

function tokenText(event: AgentEvent): string {
  if (event.event !== "agent.token") return "";
  const text = event.payload?.["text"];
  return typeof text === "string" ? text : "";
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

function objectField(payload: Record<string, unknown>, key: string): Record<string, unknown> {
  const value = payload[key];
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
}

function pathText(value: unknown): string {
  const joined = pathSegments(value).join("/");
  return joined || "none";
}

function pathList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map(pathText).filter((item) => item !== "none");
}

function isPlanMarkdownPath(
  planId: string,
  planRef: unknown,
  planPath: unknown,
): boolean {
  const planRefSegments = pathSegments(planRef);
  const planPathSegments = pathSegments(planPath);
  const planRefWorkspace = workspaceIdKey(planRef);
  const planPathWorkspace = workspaceIdKey(planPath);
  return (
    planRefSegments.length === 2 &&
    planRefSegments[0] === "plans" &&
    planRefSegments[1] === planId &&
    planPathSegments.length === 3 &&
    planPathSegments[0] === planRefSegments[0] &&
    planPathSegments[1] === planRefSegments[1] &&
    planPathSegments[2] === "plan.md" &&
    planRefWorkspace !== null &&
    planRefWorkspace === planPathWorkspace
  );
}

function workspaceIdKey(value: unknown): string | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  if (!("workspace_id" in record)) return null;
  return JSON.stringify(record["workspace_id"]);
}

function formatTime(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "";
  return new Date(value).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

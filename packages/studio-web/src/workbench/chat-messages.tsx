import { createContext, useContext, useState } from "react";
import MarkdownPreview from "@uiw/react-markdown-preview";
import rehypeSanitize from "rehype-sanitize";
import type { AgentErrorType } from "@budn/app-server-protocol";
import {
  ThreadPrimitive,
  MessagePrimitive,
  useMessage,
} from "@assistant-ui/react";
import type {
  DataMessagePartProps,
  TextMessagePartProps,
} from "@assistant-ui/react";
import { markdownSanitizeSchema } from "../viewers/markdown-security";
import type { AgentEvent, AgentSearchSource } from "./chat-zone";
import { pathSegments } from "./path-utils";

const mdWrapperElement = { "data-color-mode": "dark" } as const;

const ChatBodyCtx = createContext<{
  planActionDisabled: boolean;
  onOpenPlan?: (path: unknown) => void;
  onRunPlan?: (plan: PlanRunAction) => void;
}>({ planActionDisabled: false });

export function ChatBody(props: {
  llmConfigured: boolean;
  planActionDisabled: boolean;
  onOpenPlan?: (path: unknown) => void;
  onRunPlan?: (plan: PlanRunAction) => void;
}) {
  return (
    <ChatBodyCtx.Provider
      value={{
        planActionDisabled: props.planActionDisabled,
        onOpenPlan: props.onOpenPlan,
        onRunPlan: props.onRunPlan,
      }}
    >
      <ThreadPrimitive.Viewport
        className="chat-body"
        data-testid="chat-body"
        autoScroll
      >
        <ThreadPrimitive.If empty>
          {props.llmConfigured ? <WelcomeEmptyState /> : <LlmSetupGuide />}
        </ThreadPrimitive.If>
        <ThreadPrimitive.Messages>
          {() => <ChatMessageItem />}
        </ThreadPrimitive.Messages>
        <ThinkingIndicator />
        <div data-testid="chat-scroll-anchor" />
      </ThreadPrimitive.Viewport>
    </ChatBodyCtx.Provider>
  );
}

function ChatMessageItem() {
  const role = useMessage((s) => s.role);
  const cls = role === "user" ? "msg user" : "msg agent";
  return (
    <MessagePrimitive.Root className={cls}>
      <div className="who">
        <b>{role}</b>
      </div>
      <MessagePrimitive.Content
        components={{
          Text: MarkdownText,
          data: {
            by_name: {
              "agent.error": AgentErrorPart,
              "agent.plan_saved": PlanSavedPart,
              "agent.reasoning": ReasoningPart,
              "agent.search_sources": SearchSourcesPart,
            },
            Fallback: AgentEventPart,
          },
        }}
      />
    </MessagePrimitive.Root>
  );
}

function MarkdownText(props: TextMessagePartProps) {
  return (
    <div className="bubble">
      <MarkdownPreview
        source={props.text}
        wrapperElement={mdWrapperElement}
        className="chat-markdown"
        rehypePlugins={[[rehypeSanitize, markdownSanitizeSchema]]}
      />
    </div>
  );
}

function AgentEventPart(props: DataMessagePartProps) {
  const { planActionDisabled } = useContext(ChatBodyCtx);
  const event = reconstructAgentEvent(props.name, props.data);
  return <AgentEventRow event={event} actionDisabled={planActionDisabled} />;
}

function AgentErrorPart(props: DataMessagePartProps) {
  const event = reconstructAgentEvent(props.name, props.data);
  return <AgentEventRow event={event} />;
}

function PlanSavedPart(props: DataMessagePartProps) {
  const { planActionDisabled, onOpenPlan, onRunPlan } = useContext(ChatBodyCtx);
  const event = reconstructAgentEvent(props.name, props.data);
  return (
    <AgentEventRow
      event={event}
      actionDisabled={planActionDisabled}
      onOpenPlan={onOpenPlan}
      onRunPlan={onRunPlan}
    />
  );
}

function ReasoningPart(props: DataMessagePartProps) {
  const text = stringField(props.data, "text");
  return (
    <div className="agent-reasoning" data-testid="agent-reasoning">
      <div className="agent-reasoning__head">
        <span className="agent-reasoning__pulse" />
        <span>Thinking</span>
      </div>
      <div className="agent-reasoning__body">{text}</div>
    </div>
  );
}

export function SearchSourcesPart(props: DataMessagePartProps) {
  const sources = searchSourcesFromData(props.data);
  if (sources.length === 0) return null;
  return (
    <div className="agent-search-sources" data-testid="agent-search-sources">
      <div className="agent-search-sources__title">Sources</div>
      <ol>
        {sources.map((source, index) => (
          <li key={`${source.url}-${index}`}>
            <a href={source.url} target="_blank" rel="noreferrer">
              {source.title || source.url}
            </a>
          </li>
        ))}
      </ol>
    </div>
  );
}

function searchSourcesFromData(
  data: Record<string, unknown>,
): AgentSearchSource[] {
  const rawSources = data["sources"];
  if (!Array.isArray(rawSources)) return [];
  return rawSources.filter(isRenderableSearchSource);
}

function isRenderableSearchSource(value: unknown): value is AgentSearchSource {
  if (!value || typeof value !== "object") return false;
  const source = value as Partial<AgentSearchSource>;
  return (
    typeof source.url === "string" &&
    isHttpUrl(source.url) &&
    (source.title === undefined || typeof source.title === "string")
  );
}

function isHttpUrl(url: string): boolean {
  return url.startsWith("https://") || url.startsWith("http://");
}

function reconstructAgentEvent(
  name: string,
  data: Record<string, unknown>,
): AgentEvent {
  const { event: eventName, ...payload } = data;
  return {
    event: typeof eventName === "string" ? eventName : name,
    payload,
  };
}

function ThinkingIndicator() {
  return (
    <ThreadPrimitive.If running>
      <div className="agent-thinking" data-testid="agent-thinking">
        <span className="agent-thinking__label">Thinking</span>
        <span className="thinking-dot" />
        <span className="thinking-dot" />
        <span className="thinking-dot" />
      </div>
    </ThreadPrimitive.If>
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
        <li><code>BUDN_AGENT_OPENAI_API_KEY</code> or <code>OPENAI_API_KEY</code></li>
        <li><code>BUDN_AGENT_MODEL</code> (optional, defaults to gpt-5.2)</li>
        <li><code>BUDN_AGENT_NATIVE_WEB_SEARCH=true</code> (optional)</li>
        <li><code>BUDN_AGENT_CONFIG</code> (optional TOML config path)</li>
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

export type PlanRunAction = {
  planId: string;
  planRef: unknown;
};

export function AgentEventRow({
  event,
  actionDisabled = false,
  onOpenPlan,
  onRunPlan,
}: {
  event: AgentEvent;
  actionDisabled?: boolean;
  onOpenPlan?: (path: unknown) => void;
  onRunPlan?: (plan: PlanRunAction) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  if (event.event === "agent.error") {
    return <AgentErrorCard event={event} />;
  }
  if (event.event === "agent.done") {
    return <AgentDoneMark cancelled={event.payload?.["cancelled"] === true} />;
  }
  if (event.event === "agent.plan_saved") {
    const plan = parsePlanSavedEvent(event);
    if (plan) {
      return (
        <PlanPackageCard
          plan={plan}
          actionDisabled={actionDisabled}
          onOpenPlan={onOpenPlan}
          onRunPlan={onRunPlan}
        />
      );
    }
  }
  const label = event.event.replace("agent.", "");
  const detail = agentEventDetail(event);
  return (
    <>
      <button
        type="button"
        className="agent-op-line"
        data-testid="agent-event-row"
        onClick={() => setExpanded(true)}
      >
        <span>{label}</span>
        <code>{agentEventSummary(event, detail)}</code>
      </button>
      {expanded ? (
        <AgentEventModal
          title={label}
          detail={agentEventModalDetail(event)}
          onClose={() => setExpanded(false)}
        />
      ) : null}
    </>
  );
}

function AgentDoneMark({ cancelled }: { cancelled: boolean }) {
  return (
    <div
      className={`agent-done-mark${cancelled ? " is-cancelled" : ""}`}
      data-testid="agent-done-mark"
      aria-label={cancelled ? "assistant cancelled" : "assistant done"}
    >
      budn&apos;
    </div>
  );
}

function AgentEventModal(props: {
  title: string;
  detail: string;
  onClose: () => void;
}) {
  return (
    <div className="agent-event-modal-backdrop" data-testid="agent-event-modal">
      <div className="agent-event-modal" role="dialog" aria-modal="true">
        <div className="agent-event-modal__head">
          <span>{props.title}</span>
          <button type="button" onClick={props.onClose}>
            close
          </button>
        </div>
        <pre>{props.detail}</pre>
      </div>
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

function agentEventSummary(event: AgentEvent, detail: string): string {
  const payload = event.payload ?? {};
  const tool = stringField(payload, "tool_name");
  if (event.event === "agent.tool_start") return tool || detail || "started";
  if (event.event === "agent.tool_result")
    return tool ? `${tool} result ready` : "result ready";
  if (tool) return tool;
  if (event.event === "agent.mesh_ready") return "mesh ready";
  return detail || event.event;
}

function agentEventModalDetail(event: AgentEvent): string {
  const payload = event.payload ?? {};
  try {
    return JSON.stringify(agentEventModalPayload(event, payload), null, 2);
  } catch {
    return agentEventDetail(event);
  }
}

function agentEventModalPayload(
  event: AgentEvent,
  payload: Record<string, unknown>,
): Record<string, unknown> {
  if (event.event === "agent.tool_result") {
    return {
      event: event.event,
      ...payload,
      result: parseJsonString(stringField(payload, "result_json")),
    };
  }
  if (event.event === "agent.tool_start") {
    return {
      event: event.event,
      ...payload,
      arguments: parseJsonString(stringField(payload, "args_json")),
    };
  }
  return { event: event.event, ...payload };
}

function parseJsonString(text: string): unknown {
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
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

type FriendlyError = { title: string; hint: string };

const ERROR_MESSAGES: Record<string, FriendlyError> = {
  llm_error: {
    title: "AI service error",
    hint: "The AI service returned an error. Check the Rig OpenAI Responses configuration and try again.",
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

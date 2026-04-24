// Log panel: presentation layer for the log ring buffer held by the
// WorkbenchLayout. Data is passed in as props so the buffer never lives in
// the Zustand store (UI shell only).

export type LogLevel = "info" | "warn" | "error";

export type LogEntry = {
  id: number;
  timestamp: number;
  level: LogLevel;
  message: string;
};

type LogPanelProps = {
  entries: LogEntry[];
};

function fmtTime(value: number): string {
  const date = new Date(value);
  const hh = String(date.getHours()).padStart(2, "0");
  const mm = String(date.getMinutes()).padStart(2, "0");
  const ss = String(date.getSeconds()).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}

export function LogPanel({ entries }: LogPanelProps) {
  return (
    <ul
      className="panel__list log-panel__list"
      aria-label="log entries"
      data-testid="log-list"
    >
      {entries.length === 0 ? (
        <li className="panel__empty" data-testid="log-empty">
          no entries yet.
        </li>
      ) : (
        entries
          .slice()
          .reverse()
          .map((entry) => (
            <li
              key={entry.id}
              className={`log-panel__row log-panel__row--${entry.level}`}
              data-testid={`log-row-${entry.id}`}
            >
              <span className="log-panel__time">{fmtTime(entry.timestamp)}</span>
              <span className="log-panel__level">{entry.level}</span>
              <span className="log-panel__message">{entry.message}</span>
            </li>
          ))
      )}
    </ul>
  );
}

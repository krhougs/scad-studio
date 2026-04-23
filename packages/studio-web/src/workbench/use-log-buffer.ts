// Ring buffer hook for the log panel. Kept in React state only; never stored
// in the Zustand store (it would violate the UI-only boundary in state/ui-store).

import { useCallback, useRef, useState } from "react";
import type { LogEntry, LogLevel } from "./log-panel";

const DEFAULT_LIMIT = 50;

export type UseLogBuffer = {
  entries: LogEntry[];
  append: (level: LogLevel, message: string) => void;
  clear: () => void;
};

export function useLogBuffer(limit: number = DEFAULT_LIMIT): UseLogBuffer {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const idRef = useRef(0);

  const append = useCallback(
    (level: LogLevel, message: string) => {
      idRef.current += 1;
      const entry: LogEntry = {
        id: idRef.current,
        timestamp: Date.now(),
        level,
        message,
      };
      setEntries((prev) => {
        const next = [...prev, entry];
        if (next.length > limit) return next.slice(next.length - limit);
        return next;
      });
    },
    [limit],
  );

  const clear = useCallback(() => {
    setEntries([]);
  }, []);

  return { entries, append, clear };
}

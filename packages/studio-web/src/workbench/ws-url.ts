export type WsUrlLocation = {
  protocol: string;
  host: string;
};

export type ResolveWorkbenchWsUrlOptions = {
  envUrl?: string;
  location?: WsUrlLocation;
};

const DEFAULT_PROXY_PATH = "/app-server/ws";

export function resolveWorkbenchWsUrl(
  searchParams: URLSearchParams,
  options: ResolveWorkbenchWsUrlOptions = {},
): string {
  const fromQuery = searchParams.get("ws");
  if (fromQuery) return fromQuery;
  if (options.envUrl && options.envUrl.length > 0) return options.envUrl;

  const location = options.location ?? window.location;
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${location.host}${DEFAULT_PROXY_PATH}`;
}

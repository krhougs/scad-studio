import { useState } from "react";

export function SettingsRoute() {
  const [wsUrl, setWsUrl] = useState<string>(() => {
    const search = new URLSearchParams(window.location.search);
    return search.get("ws") ?? "ws://127.0.0.1:38421";
  });

  return (
    <section style={{ padding: "16px", fontFamily: "ui-monospace, monospace" }}>
      <h1>settings</h1>
      <p>Phase 3 占位页，完整配置界面在 Phase 7 补齐。</p>
      <label style={{ display: "block", marginTop: "12px" }}>
        WebSocket URL
        <input
          type="text"
          value={wsUrl}
          onChange={(ev) => setWsUrl(ev.target.value)}
          style={{ width: "100%", marginTop: "4px" }}
          data-testid="ws-url-input"
        />
      </label>
      <p>
        修改 URL 后请携带 <code>?ws={wsUrl}</code> 访问根路径生效。
      </p>
    </section>
  );
}

import { Link, Route, Routes } from "react-router-dom";
import { IndexRoute } from "./routes";
import { SettingsRoute } from "./routes/settings";

export function App() {
  return (
    <div>
      <nav
        style={{
          display: "flex",
          gap: "12px",
          padding: "8px 16px",
          borderBottom: "1px solid #222",
          fontFamily: "ui-monospace, monospace",
        }}
      >
        <Link to="/">workbench</Link>
        <Link to="/settings">settings</Link>
      </nav>
      <Routes>
        <Route path="/" element={<IndexRoute />} />
        <Route path="/settings" element={<SettingsRoute />} />
        <Route path="*" element={<NotFound />} />
      </Routes>
    </div>
  );
}

function NotFound() {
  return (
    <section style={{ padding: "16px", fontFamily: "ui-monospace, monospace" }}>
      <h1>404</h1>
      <p>未找到路由。</p>
    </section>
  );
}

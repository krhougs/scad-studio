import { Route, Routes } from "react-router-dom";
import { IndexRoute } from "./routes";
import { SettingsRoute } from "./routes/settings";

export function App() {
  return (
    <Routes>
      <Route path="/" element={<IndexRoute />} />
      <Route path="/settings" element={<SettingsRoute />} />
      <Route path="*" element={<NotFound />} />
    </Routes>
  );
}

function NotFound() {
  return (
    <section style={{ padding: "16px" }}>
      <h1>404</h1>
      <p>未找到路由。</p>
    </section>
  );
}

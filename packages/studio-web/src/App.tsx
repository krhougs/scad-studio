import { NavLink, Route, Routes } from "react-router-dom";
import { IndexRoute } from "./routes";
import { SettingsRoute } from "./routes/settings";

function navClass({ isActive }: { isActive: boolean }): string {
  return isActive ? "is-active" : "";
}

export function App() {
  return (
    <>
      <nav className="app-nav" aria-label="routes">
        <NavLink to="/" end className={navClass}>
          workbench
        </NavLink>
        <NavLink to="/settings" className={navClass}>
          settings
        </NavLink>
      </nav>
      <Routes>
        <Route path="/" element={<IndexRoute />} />
        <Route path="/settings" element={<SettingsRoute />} />
        <Route path="*" element={<NotFound />} />
      </Routes>
    </>
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

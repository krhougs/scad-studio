import {
  Navigate,
  Route,
  Routes,
  createSearchParams,
  useLocation,
} from "react-router-dom";
import { IndexRoute } from "./routes";

export function App() {
  return (
    <Routes>
      <Route path="/" element={<IndexRoute />} />
      <Route path="/settings" element={<SettingsRedirect />} />
      <Route path="*" element={<NotFound />} />
    </Routes>
  );
}

function SettingsRedirect() {
  const location = useLocation();
  const params = createSearchParams(location.search);
  params.set("left-panel", "settings");
  return (
    <Navigate
      to={{ pathname: "/", search: `?${params.toString()}` }}
      replace
    />
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

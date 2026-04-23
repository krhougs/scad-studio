import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import "./styles/tokens.css";
import "./styles/workbench.css";
import "./styles/workbench-zones.css";
import "./styles/primitives.css";
import "./styles/viewers.css";
import "./styles/phase7.css";

const root = document.getElementById("app");
if (!root) {
  throw new Error("missing #app mount point");
}

createRoot(root).render(
  <StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </StrictMode>,
);

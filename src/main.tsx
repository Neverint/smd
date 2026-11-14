import React from "react";
import ReactDOM from "react-dom/client";
import "./design/base.css";
import "./dev/devlog"; // DEVLOG
import { App } from "./app/App";
import { initI18n } from "./i18n";

initI18n();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

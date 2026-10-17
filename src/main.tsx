import React from "react";
import ReactDOM from "react-dom/client";
import "./i18n";
import "./styles.css";

async function start() {
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) await import("./dev/mock");
  const { default: App } = await import("./App");
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void start();

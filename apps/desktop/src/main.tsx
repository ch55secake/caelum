import React from "react";
import ReactDOM from "react-dom/client";

import App from "./app/App";
import "./ui/styles/styles.css";
import { initializeAppearance } from "./application/useSettings.js";

initializeAppearance();

const root = ReactDOM.createRoot(document.getElementById("root")!);
if (import.meta.env.MODE === "profile") {
  void import("./platform/profiling.js").then(({ startProfiling }) => {
    const onRender = startProfiling();
    root.render(
      <React.Profiler id="app" onRender={onRender}>
        <App />
      </React.Profiler>,
    );
  });
} else {
  root.render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

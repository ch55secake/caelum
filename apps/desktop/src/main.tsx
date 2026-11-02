import React from "react";
import ReactDOM from "react-dom/client";

import App from "./app/App";
import "./ui/styles/styles.css";
import { initializeAppearance } from "./application/useSettings.js";

initializeAppearance();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import "./styles.css";
import { initializeAppearance } from "./useSettings.js";

initializeAppearance();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig(({ mode }) => ({
  plugins: [react(), tailwindcss()],
  // React's normal production build deliberately disables Profiler callbacks.
  resolve:
    mode === "profile"
      ? { alias: { "react-dom/client": "react-dom/profiling" } }
      : undefined,
  build: { outDir: mode === "profile" ? "dist-profile" : "dist" },
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
}));

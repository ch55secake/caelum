import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const monacoEsm = decodeURIComponent(new URL("./node_modules/monaco-editor/esm/", import.meta.url).pathname)
  .replace(/^\/([A-Za-z]:\/)/, "$1");

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: {
    alias: [{ find: /^monaco-editor\/esm\//, replacement: `${monacoEsm}/` }],
  },
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
});

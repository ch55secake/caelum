import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "workspace.preview.ts",
  workers: 1,
  outputDir: "../preview-results/driver",
  use: {
    browserName: "webkit",
    baseURL: "http://127.0.0.1:1432",
    viewport: { width: 1280, height: 1000 },
    deviceScaleFactor: 1,
  },
  webServer: {
    command: "pnpm dev --port 1432",
    url: "http://127.0.0.1:1432",
    reuseExistingServer: false,
  },
});

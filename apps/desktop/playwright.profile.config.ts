import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./profile",
  testMatch: "*.spec.ts",
  outputDir: "profile-results/ui",
  timeout: 180_000,
  workers: 1,
  repeatEach: 3,
  reporter: [["list"]],
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "webkit", use: { browserName: "webkit" } },
  ],
  use: {
    baseURL: "http://127.0.0.1:1431",
    viewport: { width: 1440, height: 960 },
  },
  webServer: {
    command: "pnpm exec vite preview --outDir dist-profile --host 127.0.0.1 --port 1431 --strictPort",
    url: "http://127.0.0.1:1431",
    reuseExistingServer: false,
  },
});

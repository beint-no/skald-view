import { defineConfig, devices } from "@playwright/test";
export default defineConfig({
  testDir: "./tests/browser",
  timeout: 45000,
  fullyParallel: true,
  use: { baseURL: process.env.SKALD_VIEW_URL ?? "http://127.0.0.1:4173" },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: {
    command: "npm run dev",
    url: process.env.SKALD_VIEW_URL ?? "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
  },
});

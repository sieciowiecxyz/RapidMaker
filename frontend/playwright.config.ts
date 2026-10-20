import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  workers: 3,
  timeout: 45000,
  expect: { timeout: 7000 },
  reporter: 'list',
  outputDir: './tests/artifacts/results',
  use: {
    baseURL: 'http://127.0.0.1:18431',
    browserName: 'chromium',
    viewport: { width: 1440, height: 1000 },
    launchOptions: { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE },
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'npm run preview -- --port 18431',
    url: 'http://127.0.0.1:18431',
    reuseExistingServer: !process.env.CI,
  },
});

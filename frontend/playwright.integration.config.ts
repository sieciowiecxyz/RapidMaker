import { defineConfig } from '@playwright/test';

// Launched by tests/run-integration.py with a disposable DB/storage and separate API port.
export default defineConfig({
  testDir: './tests/integration',
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 20_000 },
  reporter: 'list',
  outputDir: './tests/artifacts/integration',
  use: {
    baseURL: 'http://localhost:18437',
    actionTimeout: 20_000,
    navigationTimeout: 20_000,
    browserName: 'chromium',
    viewport: { width: 1440, height: 1000 },
    launchOptions: { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE },
    screenshot: 'only-on-failure',
    // Traces can contain cookies, addresses and payment client secrets.
    trace: 'off',
  },
  webServer: {
    command: 'npm run preview -- --host localhost --port 18437 --strictPort',
    url: 'http://localhost:18437',
    reuseExistingServer: false,
    env: { RUST_API_PORT: '18436' },
  },
});

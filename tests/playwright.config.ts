import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  timeout: 240_000, // 4 min — WebLLM model download can be slow.
  expect: { timeout: 120_000 },
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL: 'http://localhost:8000',
    headless: true,
    // Service workers work most reliably in Chromium.
    // The dist/ bundle is browser-WASM + SW-based.
  },
  webServer: {
    command: 'python3 -m http.server --directory ../dist 8000',
    port: 8000,
    timeout: 60_000,
    reuseExistingServer: true,
  },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' },
    },
  ],
});

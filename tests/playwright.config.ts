import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  timeout: 240_000, // 4 min — WebLLM model download can be slow.
  expect: { timeout: 120_000 },
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL: 'http://localhost:8001',
    headless: true,
    // Service workers work most reliably in Chromium.
    // The pkg/ bundle is browser-WASM + SW-based.
  },
  webServer: {
    command: 'python3 -m http.server --directory ../pkg 8001',
    port: 8001,
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

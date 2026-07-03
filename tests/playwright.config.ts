import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  timeout: 1_200_000, // 20 min — WebLLM model download is ~1.2 GB on cold runs.
  expect: { timeout: 120_000 },
  fullyParallel: false,
  workers: 1,
  use: {
    baseURL: 'http://localhost:8001',
    // WebLLM requires WebGPU, which isn't available in headless Chromium on
    // Linux. Run headed so the LLM can load. CI will need a real GPU or an
    // env-var override to flip back to headless + skip the LLM steps.
    headless: process.env.HEADED ? false : true,
    // Service workers work most reliably in Chromium.
    // The pkg/ bundle is browser-WASM + SW-based.
  },
  webServer: {
    // serve_pkg.py sends Cache-Control: no-store — the persistent chromium
    // profile otherwise disk-caches unhashed tool.js/tool-audio.js and tests
    // run stale JS after a page regen.
    command: 'python3 serve_pkg.py ../pkg 8001',
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

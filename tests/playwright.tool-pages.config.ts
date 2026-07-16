import { defineConfig } from '@playwright/test';

// Config for the FULL standalone tool-page suite (tool-page-*.spec.ts, ~393
// specs). The default playwright.config.ts inherited workers:1 /
// fullyParallel:false / timeout:20min from a WebLLM smoke test whose model
// download needs those; running the whole tool-page corpus under it is serial
// and effectively untestable in CI, so only calculator+clock (tool_pages.spec.ts)
// ran on main. These are fast, pure in-browser compute pages with no model
// download, so give them real parallelism and a tight per-test timeout.
//
// Used by the nightly workflow (.github/workflows/nightly-blocks.yml). The
// on-PR / on-main lightweight Playwright runs keep using playwright.config.ts.
export default defineConfig({
  testDir: '.',
  // Only the per-tool page specs — never the WebLLM/tool_pages smoke specs.
  testMatch: /tool-page-.*\.spec\.ts$/,
  timeout: 60_000, // per test — these pages compute locally in well under a second.
  expect: { timeout: 15_000 },
  fullyParallel: true,
  // A fixed, override-able worker count. GitHub's ubuntu-latest runner has a
  // handful of vCPUs; 4 keeps each worker responsive without oversubscribing.
  workers: process.env.PW_WORKERS ? Number(process.env.PW_WORKERS) : 4,
  use: {
    baseURL: 'http://localhost:8001',
    headless: true,
  },
  webServer: {
    // serve_pkg.py sends Cache-Control: no-store so workers never run stale JS
    // after a page regen. One server is shared across all workers.
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

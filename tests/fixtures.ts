// Persistent-context test fixture.
//
// WebLLM downloads ~600 MB–1.2 GB of model shards on first run and caches
// them in OPFS / IndexedDB / Cache Storage. Playwright's default per-test
// context wipes that cache between tests AND between runs. Switching to a
// persistent context backed by a fixed user-data directory makes the cache
// survive both — first run pays the download cost, subsequent runs (and the
// second test in the same run) skip straight to "Ready".
//
// Fixture is worker-scoped so all tests in a single worker share one
// persistent browser. Repo-relative `.cache/chromium-profile/` is gitignored.

import { test as base, chromium, type BrowserContext } from '@playwright/test';
import path from 'node:path';

const USER_DATA_DIR = path.resolve(__dirname, '.cache/chromium-profile');

export const test = base.extend<object, { persistentContext: BrowserContext }>({
  persistentContext: [
    async ({}, use) => {
      const ctx = await chromium.launchPersistentContext(USER_DATA_DIR, {
        headless: process.env.HEADED ? false : true,
      });
      await use(ctx);
      await ctx.close();
    },
    { scope: 'worker' },
  ],
  context: async ({ persistentContext }, use) => {
    await use(persistentContext);
  },
  page: async ({ persistentContext }, use) => {
    const page = persistentContext.pages()[0] ?? (await persistentContext.newPage());
    await use(page);
  },
});

export { expect } from '@playwright/test';

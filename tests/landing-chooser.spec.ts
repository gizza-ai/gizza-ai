// landing-chooser.spec.ts — chooser renders without SW/WebGPU and links out.
// The apex `/` is a static HTML page (no Service Worker, no runtime) that
// presents two buttons routing to /chat and /tools/. These tests assert the
// chooser structure without touching the LLM or WebGPU at all.
//
// The chooser test runs in a fresh ephemeral browser context with
// serviceWorkers: 'block' so a previously-registered SW from other test runs
// can never intercept GET / and return the full chat UI. This makes the
// assertion deterministic regardless of local SW lifecycle state.
import { test, expect, chromium } from '@playwright/test';

const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8001';

test('/ shows two buttons linking to /chat and /tools/', async () => {
  // Use a fresh ephemeral context with SWs blocked so stale SW state from
  // prior runs cannot intercept the request and serve the chat UI instead.
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ serviceWorkers: 'block', baseURL: BASE_URL });
  const page = await ctx.newPage();
  try {
    await page.goto('/');
    await expect(page.locator('.chooser__btn[href="/chat"]')).toBeVisible();
    await expect(page.locator('.chooser__btn[href="/tools/"]')).toBeVisible();
    await expect(page.locator('.chooser__buttons')).toBeVisible();
    // Confirm we are NOT on the chat UI (no #composer element)
    await expect(page.locator('#composer')).not.toBeAttached();
  } finally {
    await ctx.close();
    await browser.close();
  }
});

test('tools button reaches a working tool without the runtime', async () => {
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ serviceWorkers: 'block', baseURL: BASE_URL });
  const page = await ctx.newPage();
  try {
    await page.goto('/tools/');
    await expect(page).toHaveTitle(/Tools/);
  } finally {
    await ctx.close();
    await browser.close();
  }
});

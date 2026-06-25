// landing-chooser.spec.ts — chooser renders without SW/WebGPU and links out.
// The apex `/` is a static HTML page (no Service Worker, no runtime) that
// presents two buttons routing to /chat and /tools/. These tests assert the
// chooser structure without touching the LLM or WebGPU at all.
//
// NOTE: In CI the SW is bypassed for `/` via solobase `extra_bypass_exact`.
// In local dev against pkg/ served by http.server a previously-registered SW
// may intercept `/` and return the full chat UI; the `.chooser__btn` selector
// asserts we are on the real static chooser page.
import { test, expect } from './fixtures';

test('apex shows two-button chooser and links to chat + tools', async ({ page }) => {
  await page.goto('/');
  // Use .chooser__btn to target the static chooser buttons, not the nav link
  // that the SW-rendered chat UI also exposes with name "Chat".
  await expect(page.locator('.chooser__btn[href="/chat"]')).toBeVisible();
  await expect(page.locator('.chooser__btn[href="/tools/"]')).toBeVisible();
  await expect(page.locator('.chooser__buttons')).toBeVisible();
});

test('tools button reaches a working tool without the runtime', async ({ page }) => {
  await page.goto('/tools/');
  await expect(page).toHaveTitle(/Tools/);
});

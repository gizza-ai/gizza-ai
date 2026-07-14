import { test, expect } from './fixtures';

// Tool pages render to pkg/tools/<sub>/ (output of the generator). The Playwright
// webServer (playwright.config.ts) serves ../pkg, so these resolve at
// /tools/<slug>/. Tool pages are served at apex paths (no subdomains); the
// runtime Service Worker bypasses /tools/ (see js/sw-bypass.test.js).

test.describe('standalone tool pages', () => {
  test('calculator page computes and has SEO tags', async ({ page }) => {
    await page.goto('/tools/calculator/');

    await expect(page).toHaveTitle(/Free Online Calculator/);
    await expect(page.locator('meta[name="description"]')).toHaveAttribute('content', /browser/i);
    await expect(page.locator('script[type="application/ld+json"]')).toHaveCount(1);

    await expect(page.locator('.tool-brand')).toHaveText('Tools');
    await expect(page.locator('.tool-brand')).toHaveAttribute('href', '/tools/');
    await expect(page.locator('.tool-footer')).toContainText(
      'Free, private, in-browser tools — everything runs locally, nothing is uploaded.',
    );

    await page.fill('#in-expr', '2 + 2 * 3');
    await expect(page.locator('#tool-output')).toHaveText('8', { timeout: 10_000 });
  });

  test('clock page shows a live UTC timestamp', async ({ page }) => {
    await page.goto('/tools/clock/');
    await expect(page).toHaveTitle(/Current UTC Time/);
    await expect(page.locator('#tool-output')).toHaveText(
      /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|\+00:00)$/,
      { timeout: 10_000 },
    );
  });
});

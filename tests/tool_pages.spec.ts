import { test, expect } from './fixtures';

// Tool pages render to pkg/tools/<sub>/ (output of the generator). The Playwright
// webServer (playwright.config.ts) serves ../pkg, so these resolve at
// /tools/<sub>/. The *.gizza.ai subdomain rewrite is covered by
// functions/routing.test.mjs (node test), not here.

test.describe('standalone tool pages', () => {
  test('calculator page computes and has SEO tags', async ({ page }) => {
    await page.goto('/tools/calculator/');

    await expect(page).toHaveTitle(/Free Online Calculator/);
    await expect(page.locator('meta[name="description"]')).toHaveAttribute('content', /browser/i);
    await expect(page.locator('script[type="application/ld+json"]')).toHaveCount(1);

    await expect(page.locator('.tool-brand')).toContainText('gizza.ai');
    await expect(page.locator('.tool-footer')).toContainText('Powered by gizza.ai');

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

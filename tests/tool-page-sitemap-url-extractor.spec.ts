import { test, expect } from './fixtures';

const SITEMAP = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/</loc><lastmod>2026-01-01</lastmod></url>
  <url><loc>https://example.com/about</loc></url>
</urlset>`;

test('sitemap-url-extractor page', async ({ page }) => {
  await page.goto('/tools/sitemap-url-extractor/');
  await page.fill('#in-xml', SITEMAP);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 URL(s) [urlset]:', { timeout: 15000 });
  await expect(out).toContainText('https://example.com/\t2026-01-01');
  await expect(out).toContainText('https://example.com/about');
});

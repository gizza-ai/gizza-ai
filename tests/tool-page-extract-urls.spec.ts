import { test, expect } from './fixtures';

// /tools/extract-urls/ extracts + dedupes URLs in-browser (pure wasm).
test('extract-urls lists unique URLs', async ({ page }) => {
  await page.goto('/tools/extract-urls/');
  await page.fill(
    '#in-text',
    'See https://example.com/a and (http://b.io/c). Dup https://example.com/a again.',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 unique URL(s)', { timeout: 15000 });
  await expect(out).toContainText('https://example.com/a');
  await expect(out).toContainText('http://b.io/c');
});

test('extract-urls splits components when checked', async ({ page }) => {
  await page.goto('/tools/extract-urls/');
  await page.fill('#in-text', 'go to https://host.com:8443/p/q?a=1#frag now');
  await page.check('#in-split_components');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('host:   host.com', { timeout: 15000 });
  await expect(out).toContainText('port:   8443');
  await expect(out).toContainText('query:  a=1');
  await expect(out).toContainText('fragment: frag');
});

import { test, expect } from './fixtures';

// /tools/parse-uri/ splits a URI/URL into RFC 3986 components in-browser (pure wasm).
test('parse-uri splits a full https URL', async ({ page }) => {
  await page.goto('/tools/parse-uri/');
  await page.fill(
    '#in-uri',
    'https://user:pass@www.Example.COM:8443/docs/report.pdf?q=hello+world&lang=en#section-2',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('absolute URI', { timeout: 15000 });
  await expect(out).toContainText('https');
  // host is lowercased
  await expect(out).toContainText('www.example.com');
  // origin (normalized, userinfo dropped) + filename + extension
  await expect(out).toContainText('https://www.example.com:8443');
  await expect(out).toContainText('report.pdf');
  await expect(out).toContainText('8443');
  await expect(out).toContainText('user');
  await expect(out).toContainText('pass');
  // query params decoded ('+' -> space)
  await expect(out).toContainText('q = hello world');
  await expect(out).toContainText('lang = en');
  await expect(out).toContainText('section-2');
});

test('parse-uri handles a relative reference', async ({ page }) => {
  await page.goto('/tools/parse-uri/');
  await page.fill('#in-uri', '/search?q=rust&page=2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('relative reference', { timeout: 15000 });
  await expect(out).toContainText('/search');
  await expect(out).toContainText('q = rust');
  await expect(out).toContainText('page = 2');
});

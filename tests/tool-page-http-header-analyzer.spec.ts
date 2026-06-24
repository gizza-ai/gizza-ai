import { test, expect } from './fixtures';

// /tools/http-header-analyzer/ explains pasted HTTP response headers in-browser (pure wasm).
test('http-header-analyzer explains headers and lists missing security headers', async ({ page }) => {
  await page.goto('/tools/http-header-analyzer/');
  await page.fill(
    '#in-headers',
    'HTTP/2 200\ncontent-type: text/html; charset=utf-8\ncache-control: public, max-age=3600\ncontent-encoding: gzip\nstrict-transport-security: max-age=31536000; includeSubDomains\nserver: nginx/1.25.0\n',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Security grade:', { timeout: 15000 });
  await expect(out).toContainText('Status line: HTTP/2 200');
  await expect(out).toContainText('cache-control');
  await expect(out).toContainText('reused for 3600 seconds');
  await expect(out).toContainText("compressed with 'gzip'");
  // HSTS is present so it must NOT be in the missing list, but CSP must be.
  await expect(out).toContainText('Missing recommended security headers');
  await expect(out).toContainText('Content-Security-Policy');
  await expect(out).toContainText('X-Content-Type-Options');
});

test('http-header-analyzer flags Set-Cookie hardening gaps', async ({ page }) => {
  await page.goto('/tools/http-header-analyzer/');
  await page.fill('#in-headers', 'set-cookie: id=abc; Path=/\n');
  const out = page.locator('#tool-output');
  await expect(out).toContainText("missing 'HttpOnly'", { timeout: 15000 });
  await expect(out).toContainText("missing 'Secure'");
  await expect(out).toContainText("no 'SameSite'");
});

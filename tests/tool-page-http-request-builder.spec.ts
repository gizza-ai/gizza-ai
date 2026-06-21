import { test, expect } from './fixtures';

// /tools/http-request-builder/ builds a raw HTTP request in-browser (pure wasm).
test('builds a POST request with auto Host and Content-Length', async ({ page }) => {
  await page.goto('/tools/http-request-builder/');
  await page.fill('#in-url', 'https://api.example.com/v1/items?limit=10');
  await page.selectOption('#in-method', 'POST');
  await page.fill('#in-headers', 'Accept: application/json');
  await page.fill('#in-body', '{"name":"widget"}');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('POST /v1/items?limit=10 HTTP/1.1', { timeout: 15000 });
  await expect(out).toContainText('Host: api.example.com');
  await expect(out).toContainText('Accept: application/json');
  await expect(out).toContainText('Content-Length: 17');
  await expect(out).toContainText('{"name":"widget"}');
});

test('simple GET with default method', async ({ page }) => {
  await page.goto('/tools/http-request-builder/');
  await page.fill('#in-url', 'https://example.com/path');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('GET /path HTTP/1.1', { timeout: 15000 });
  await expect(out).toContainText('Host: example.com');
});

import { test, expect } from './fixtures';

const sample = 'GET /v1/items?page=2 HTTP/1.1\nhost:   api.example.com\nACCEPT: application/json\naccept: text/plain\nx-request-id:   9f3c\ncontent-type:application/json';

test('http-header-normalizer canonicalizes, trims, sorts, and combines defaults', async ({ page }) => {
  await page.goto('/tools/http-header-normalizer/');
  await page.fill('#in-input', sample);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('GET /v1/items?page=2 HTTP/1.1', { timeout: 15000 });
  await expect(out).toContainText('Accept: application/json, text/plain');
  await expect(out).toContainText('Content-Type: application/json');
  await expect(out).toContainText('Host: api.example.com');
  await expect(out).toContainText('X-Request-ID: 9f3c');
});

test('http-header-normalizer supports query params for lowercase ordered output', async ({ page }) => {
  await page.goto('/tools/http-header-normalizer/?input=Host%3A%20api.example.com%0AContent-Type%3A%20application%2Fjson%0AETag%3A%20%22abc%22&case=lower&sort=none&duplicates=combine&unfold=true&drop_empty=false&output=headers');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('host: api.example.com', { timeout: 15000 });
  await expect(out).toContainText('content-type: application/json');
  await expect(out).toContainText('etag: "abc"');
});

test('http-header-normalizer exposes filters and alternate output modes', async ({ page }) => {
  await page.goto('/tools/http-header-normalizer/');
  await page.fill('#in-input', 'host: api.example.com\nx-private-token: example\nx-empty:\naccept: application/json');
  await page.fill('#in-drop_headers', 'x-private-token');
  await page.check('#in-drop_empty');
  await page.selectOption('#in-output', 'summary');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('metric,value', { timeout: 15000 });
  await expect(out).toContainText('names_dropped,2');
});

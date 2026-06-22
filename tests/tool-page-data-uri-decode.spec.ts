import { test, expect } from './fixtures';

// /tools/data-uri-decode/ decodes a data: URI (RFC 2397) into its MIME type,
// charset, encoding and payload — entirely in-browser (pure wasm).
test('data-uri-decode decodes a base64 text payload', async ({ page }) => {
  await page.goto('/tools/data-uri-decode/');
  await page.fill(
    '#in-uri',
    'data:text/plain;charset=utf-8;base64,SGVsbG8sIFdvcmxkIQ==',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('text/plain', { timeout: 15000 });
  await expect(out).toContainText('charset:     utf-8');
  await expect(out).toContainText('encoding:    base64');
  await expect(out).toContainText('Decoded text:');
  await expect(out).toContainText('Hello, World!');
});

test('data-uri-decode handles percent-encoded non-ASCII text', async ({ page }) => {
  await page.goto('/tools/data-uri-decode/');
  await page.fill('#in-uri', 'data:text/plain;charset=utf-8,Hello%20%E4%B8%96%E7%95%8C');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('encoding:    url', { timeout: 15000 });
  await expect(out).toContainText('Hello 世界');
});

test('data-uri-decode reports a binary payload with detected file type', async ({ page }) => {
  await page.goto('/tools/data-uri-decode/');
  await page.fill('#in-uri', 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('image/png', { timeout: 15000 });
  await expect(out).toContainText('detected:    image/png');
  await expect(out).toContainText('Hex preview:');
  await expect(out).toContainText('89504e47');
});

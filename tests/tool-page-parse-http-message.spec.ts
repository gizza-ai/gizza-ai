import { test, expect } from './fixtures';

// /tools/parse-http-message/ decodes a raw HTTP/1.x message in-browser (pure wasm).
test('parse-http-message decodes an HTTP request', async ({ page }) => {
  await page.goto('/tools/parse-http-message/');
  await page.fill(
    '#in-message',
    'GET /index.html?q=1&lang=en HTTP/1.1\nHost: example.com\nUser-Agent: curl/8.0\nAccept: */*\n\n',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('HTTP request', { timeout: 15000 });
  await expect(out).toContainText('GET');
  await expect(out).toContainText('/index.html');
  await expect(out).toContainText('q=1&lang=en');
  await expect(out).toContainText('Host: example.com');
  await expect(out).toContainText('HTTP/1.1');
});

test('parse-http-message decodes an HTTP response with a body', async ({ page }) => {
  await page.goto('/tools/parse-http-message/');
  await page.fill(
    '#in-message',
    'HTTP/1.1 404 Not Found\nContent-Type: text/html; charset=utf-8\nContent-Length: 9\n\nNot here!',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('HTTP response', { timeout: 15000 });
  await expect(out).toContainText('404 Not Found');
  await expect(out).toContainText('Client Error');
  await expect(out).toContainText('text/html; charset=utf-8');
  await expect(out).toContainText('Not here!');
});

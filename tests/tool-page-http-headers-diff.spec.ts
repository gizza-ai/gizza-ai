import { test, expect } from './fixtures';

const LEFT = 'Content-Type: text/html\nServer: nginx\nX-Frame-Options: DENY';
const RIGHT = 'Content-Type: application/json\nServer: nginx\nCache-Control: no-cache';

const REPORT = `Header diff — 1 added, 1 removed, 1 changed, 1 unchanged

Added (1):
  + Cache-Control: no-cache

Removed (1):
  - X-Frame-Options: DENY

Changed (1):
  ~ Content-Type: text/html -> application/json

Unchanged (1):
  Server`;

test('http-headers-diff reports added removed and changed headers', async ({ page }) => {
  await page.goto('/tools/http-headers-diff/');
  await page.fill('#in-left', LEFT);
  await page.fill('#in-right', RIGHT);
  await page.selectOption('#in-output', 'report');

  await expect(page.locator('#tool-output')).toHaveText(REPORT, { timeout: 15000 });
});

test('http-headers-diff deep-link ignores noisy headers', async ({ page }) => {
  const left = 'Content-Type: text/html\nDate: Mon, 01 Jan 2024 00:00:00 GMT\nAge: 0';
  const right = 'Content-Type: text/html\nDate: Tue, 02 Jan 2024 00:00:00 GMT\nAge: 42';
  await page.goto(
    '/tools/http-headers-diff/?left=' +
      encodeURIComponent(left) +
      '&right=' +
      encodeURIComponent(right) +
      '&ignore=Date%2C%20Age&ignore_order=false&output=report',
  );

  await expect(page.locator('#in-left')).toHaveValue(left, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText(
    `Header diff — 0 added, 0 removed, 0 changed, 1 unchanged

Added (0):
  (none)

Removed (0):
  (none)

Changed (0):
  (none)

Unchanged (1):
  Content-Type`,
    { timeout: 15000 },
  );
});

test('http-headers-diff can ignore comma-token ordering', async ({ page }) => {
  await page.goto('/tools/http-headers-diff/');
  await page.fill('#in-left', 'Cache-Control: no-cache, no-store');
  await page.fill('#in-right', 'Cache-Control: no-store, no-cache');
  await page.check('#in-ignore_order');

  await expect(page.locator('#tool-output')).toHaveText(
    `Header diff — 0 added, 0 removed, 0 changed, 1 unchanged

Added (0):
  (none)

Removed (0):
  (none)

Changed (0):
  (none)

Unchanged (1):
  Cache-Control`,
    { timeout: 15000 },
  );
});

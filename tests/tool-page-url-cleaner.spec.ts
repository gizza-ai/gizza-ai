import { test, expect } from './fixtures';

test('url-cleaner page strips utm + click ids', async ({ page }) => {
  await page.goto('/tools/url-cleaner/');
  await page.fill('#in-url', 'https://example.com/p?utm_source=x&id=42&fbclid=abc');
  await expect(page.locator('#tool-output')).toHaveText('https://example.com/p?id=42', {
    timeout: 15000,
  });
});

test('url-cleaner drops the ? when every param is tracking', async ({ page }) => {
  await page.goto('/tools/url-cleaner/');
  await page.fill('#in-url', 'https://example.com/p?utm_source=x&gclid=z');
  await expect(page.locator('#tool-output')).toHaveText('https://example.com/p', {
    timeout: 15000,
  });
});

test('url-cleaner extra param + per_line batch', async ({ page }) => {
  await page.goto('/tools/url-cleaner/');
  await page.fill('#in-url', 'https://a.com?sid=9&p=1\nhttps://b.com?utm_term=t');
  await page.fill('#in-extra', 'sid');
  await page.check('#in-per_line');
  await expect(page.locator('#tool-output')).toHaveText('https://a.com?p=1\nhttps://b.com', {
    timeout: 15000,
  });
});

test('url-cleaner query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/url-cleaner/?url=' +
      encodeURIComponent('https://example.com/x?utm_medium=email&keep=1'),
  );
  await expect(page.locator('#in-url')).toHaveValue(
    'https://example.com/x?utm_medium=email&keep=1',
    { timeout: 15000 },
  );
  await expect(page.locator('#tool-output')).toHaveText('https://example.com/x?keep=1', {
    timeout: 15000,
  });
});

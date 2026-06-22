import { test, expect } from './fixtures';

test('data-uri-encode page base64 default', async ({ page }) => {
  await page.goto('/tools/data-uri-encode/');

  // Default mime (text/plain) + default encoding (base64).
  await page.fill('#in-text', 'Hello, World!');
  await expect(page.locator('#tool-output')).toHaveText(
    'data:text/plain;base64,SGVsbG8sIFdvcmxkIQ==',
    { timeout: 15000 },
  );
});

test('data-uri-encode page url encoding escapes reserved chars', async ({ page }) => {
  await page.goto('/tools/data-uri-encode/');
  await page.fill('#in-text', 'a b/c?');
  await page.selectOption('#in-encoding', 'url');
  await expect(page.locator('#tool-output')).toHaveText(
    'data:text/plain,a%20b%2Fc%3F',
    { timeout: 15000 },
  );
});

test('data-uri-encode page honors a chosen mime type', async ({ page }) => {
  await page.goto('/tools/data-uri-encode/');
  await page.fill('#in-text', '<svg/>');
  await page.fill('#in-mime', 'image/svg+xml');
  await expect(page.locator('#tool-output')).toHaveText(
    'data:image/svg+xml;base64,PHN2Zy8+',
    { timeout: 15000 },
  );
});

test('data-uri-encode query-param deep-link prefills controls', async ({ page }) => {
  await page.goto(
    '/tools/data-uri-encode/?text=' +
      encodeURIComponent('hi') +
      '&encoding=url',
  );
  await expect(page.locator('#in-text')).toHaveValue('hi', { timeout: 15000 });
  await expect(page.locator('#in-encoding')).toHaveValue('url');
  await expect(page.locator('#tool-output')).toHaveText('data:text/plain,hi', {
    timeout: 15000,
  });
});

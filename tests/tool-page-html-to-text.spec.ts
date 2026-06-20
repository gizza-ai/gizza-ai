import { test, expect } from './fixtures';

test('html-to-text page strips tags to plain text', async ({ page }) => {
  await page.goto('/tools/html-to-text/');
  await page.fill('#in-html', '<h1>Title</h1><p>Hello <b>world</b>.</p>');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Hello world.', { timeout: 15000 });
  await expect(out).not.toContainText('<');
});

test('html-to-text query-param deep-link', async ({ page }) => {
  await page.goto('/tools/html-to-text/?html=' + encodeURIComponent('<p>deep &amp; link</p>'));
  await expect(page.locator('#in-html')).toHaveValue('<p>deep &amp; link</p>', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('deep & link', { timeout: 15000 });
});

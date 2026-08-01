import { test, expect } from './fixtures';

const output = (page) =>
  page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');

test('markdown-strip reduces markdown to clean plain text with defaults', async ({ page }) => {
  await page.goto('/tools/markdown-strip/');
  await page.fill('#in-text', '# Title\n\nSome **bold**, *italic* and ~~struck~~ text.');
  await expect(page.locator('#tool-output')).toContainText('Title', { timeout: 15000 });
  expect(await output(page)).toBe('Title\nSome bold, italic and struck text.');
});

test('markdown-strip deep-link renders links=both as label (url)', async ({ page }) => {
  const text = encodeURIComponent('Read [the docs](https://example.com).');
  await page.goto(`/tools/markdown-strip/?text=${text}&links=both`);
  await expect(page.locator('#in-links')).toHaveValue('both');
  await expect(page.locator('#tool-output')).toContainText('example.com', { timeout: 15000 });
  expect(await output(page)).toBe('Read the docs (https://example.com).');
});

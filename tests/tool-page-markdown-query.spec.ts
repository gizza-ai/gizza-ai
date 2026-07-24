import { test, expect } from './fixtures';

const sample = '# Title\n\nSee [Docs](https://example.com) and [API](/api).\n\n## Usage';

test('markdown-query page extracts links as exact text', async ({ page }) => {
  await page.goto('/tools/markdown-query/');
  await page.fill('#in-markdown', sample);
  await page.selectOption('#in-extract', 'links');
  await page.selectOption('#in-format', 'text');

  await expect(page.locator('#tool-output')).toHaveText(
    'Docs (https://example.com)\nAPI (/api)',
    { timeout: 15000 },
  );
});

test('markdown-query page extracts headings with line numbers', async ({ page }) => {
  await page.goto('/tools/markdown-query/');
  await page.fill('#in-markdown', sample);
  await page.selectOption('#in-extract', 'headings');
  await page.check('#in-include_line_numbers');

  await expect(page.locator('#tool-output')).toHaveText('L1\tTitle\nL5\t  Usage', {
    timeout: 15000,
  });
});

test('markdown-query query-param deep-link prefills and computes JSON images', async ({ page }) => {
  const md = 'Logo: ![Logo](img/logo.png "Logo title")';
  await page.goto(
    '/tools/markdown-query/?markdown=' +
      encodeURIComponent(md) +
      '&extract=images&format=json&include_line_numbers=true',
  );

  await expect(page.locator('#in-markdown')).toHaveValue(md, { timeout: 15000 });
  await expect(page.locator('#in-extract')).toHaveValue('images');
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#in-include_line_numbers')).toBeChecked();

  const output = page.locator('#tool-output');
  await expect(output).toContainText('"count": 1', { timeout: 15000 });
  await expect(output).toContainText('"alt": "Logo"');
  await expect(output).toContainText('"url": "img/logo.png"');
  await expect(output).toContainText('"line": 1');
});

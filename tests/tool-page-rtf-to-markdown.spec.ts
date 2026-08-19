import { test, expect } from './fixtures';

const output = (page) =>
  page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');

test('rtf-to-markdown converts headings and emphasis with defaults', async ({ page }) => {
  await page.goto('/tools/rtf-to-markdown/');
  await page.fill(
    '#in-rtf',
    '{\\rtf1\\ansi{\\pard\\outlinelevel0 Project notes\\par}This is \\b bold\\b0  and \\i italic\\i0  text.\\par}'
  );
  await expect(page.locator('#tool-output')).toContainText('Project notes', { timeout: 15000 });
  expect(await output(page)).toBe('# Project notes\n\nThis is **bold** and *italic* text.');
});

test('rtf-to-markdown deep-link renders tables as tab-separated text', async ({ page }) => {
  const rtf = encodeURIComponent('{\\rtf1\\ansi\\trowd\\intbl Name\\cell Qty\\cell\\row\\trowd\\intbl Bolt\\cell 12\\cell\\row}');
  await page.goto(`/tools/rtf-to-markdown/?rtf=${rtf}&tables=text&underline=ignore&links=false`);
  await expect(page.locator('#in-tables')).toHaveValue('text');
  await expect(page.locator('#in-underline')).toHaveValue('ignore');
  await expect(page.locator('#in-links')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('Bolt', { timeout: 15000 });
  expect(await output(page)).toBe('Name\tQty\nBolt\t12');
});

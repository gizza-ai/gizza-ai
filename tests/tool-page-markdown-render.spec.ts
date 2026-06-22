import { test, expect } from './fixtures';

test('markdown-render page renders Markdown to HTML', async ({ page }) => {
  await page.goto('/tools/markdown-render/');
  await page.fill('#in-markdown', '# Title\n\nHello **world**.');
  await expect(page.locator('#tool-output')).toHaveText(
    /<h1[\s\S]*>Title<[\s\S]*<strong>world<\/strong>/,
    { timeout: 15000 },
  );
});

test('markdown-render page renders a GFM table via query-param deep-link', async ({ page }) => {
  await page.goto(
    '/tools/markdown-render/?markdown=' +
      encodeURIComponent('| a | b |\n|---|---|\n| 1 | 2 |'),
  );
  await expect(page.locator('#in-markdown')).toHaveValue('| a | b |\n|---|---|\n| 1 | 2 |', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toHaveText(/<table>[\s\S]*<th>a<\/th>[\s\S]*<td>1<\/td>/, {
    timeout: 15000,
  });
});

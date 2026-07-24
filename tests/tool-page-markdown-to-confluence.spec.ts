import { test, expect } from './fixtures';

test('markdown-to-confluence page converts Markdown to storage format', async ({ page }) => {
  await page.goto('/tools/markdown-to-confluence/');

  await page.fill(
    '#in-input',
    '# Title\n\nSome **bold** and `code`.\n\n```sql\nSELECT 1;\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |',
  );

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<h1>Title</h1>', { timeout: 15000 });
  await expect(out).toContainText('<strong>bold</strong>');
  await expect(out).toContainText('<code>code</code>');
  await expect(out).toContainText('<ac:structured-macro ac:name="code">');
  await expect(out).toContainText('<ac:parameter ac:name="language">sql</ac:parameter>');
  await expect(out).toContainText('<table><tbody><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></tbody></table>');
});

test('markdown-to-confluence page supports wiki format and panel checkbox', async ({ page }) => {
  await page.goto('/tools/markdown-to-confluence/');

  await page.fill('#in-input', '> Note: back up first.');
  await page.selectOption('#in-format', 'wiki');
  await expect(page.locator('#tool-output')).toHaveText('{note}\nback up first.\n{note}', {
    timeout: 15000,
  });

  await page.uncheck('#in-panel_blockquotes');
  await expect(page.locator('#tool-output')).toHaveText('{quote}\nNote: back up first.\n{quote}', {
    timeout: 15000,
  });
});

test('markdown-to-confluence page applies heading_offset and deep-link params', async ({ page }) => {
  await page.goto(
    '/tools/markdown-to-confluence/?input=' +
      encodeURIComponent('# Deep') +
      '&format=storage&panel_blockquotes=true&heading_offset=2',
  );

  await expect(page.locator('#in-input')).toHaveValue('# Deep', { timeout: 15000 });
  await expect(page.locator('#in-heading_offset')).toHaveValue('2');
  await expect(page.locator('#tool-output')).toHaveText('<h3>Deep</h3>', { timeout: 15000 });
});

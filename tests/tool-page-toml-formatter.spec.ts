import { test, expect } from './fixtures';

test('toml-formatter page formats, sorts and compacts TOML', async ({ page }) => {
  await page.goto('/tools/toml-formatter/');
  await page.fill('#in-input', 'b=2\na=1\n');
  await page.selectOption('#in-sort_keys', 'asc');
  await page.selectOption('#in-spacing', 'compact');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('a=1', { timeout: 15000 });
  await expect(out).toContainText('b=2');
  await expect(out).not.toContainText('a = 1');
});

test('toml-formatter page deep-link expands arrays and preserves comments', async ({ page }) => {
  const qs =
    '?input=' + encodeURIComponent('# cfg\nvals=[1,2,3]\n') +
    '&indent=2&sort_keys=preserve&spacing=standard&array_style=expand&column_width=20&align_values=false&blank_line_before_tables=true&keep_comments=true';
  await page.goto('/tools/toml-formatter/' + qs);

  await expect(page.locator('#in-array_style')).toHaveValue('expand', { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('# cfg', { timeout: 15000 });
  await expect(out).toContainText('vals = [');
  await expect(out).toContainText('  1,');
});

test('toml-formatter page exercises checkbox and boundary values', async ({ page }) => {
  await page.goto('/tools/toml-formatter/');
  await page.fill('#in-input', '# remove\nlong = ["alpha", "bravo", "charlie", "delta"]\nshort = 1\n');
  await page.fill('#in-column_width', '20');
  await page.check('#in-align_values');
  await page.uncheck('#in-keep_comments');

  const out = page.locator('#tool-output');
  await expect(out).not.toContainText('# remove', { timeout: 15000 });
  await expect(out).toContainText('long  = [');
  await expect(out).toContainText('short = 1');
});

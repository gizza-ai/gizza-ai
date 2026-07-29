import { test, expect } from './fixtures';

test('wrap-lines-in-quotes builds a comma-separated SQL-style list', async ({ page }) => {
  await page.goto('/tools/wrap-lines-in-quotes/');
  await page.fill('#in-text', 'apple\nbanana\ncherry');
  await page.selectOption('#in-wrap', 'single');
  await page.fill('#in-separator', ',');
  await expect(page.locator('#tool-output')).toHaveText(
    "'apple',\n'banana',\n'cherry'",
    { timeout: 15000 },
  );
});

test('wrap-lines-in-quotes honours a deep link with non-default checkbox values', async ({ page }) => {
  const qs =
    '?text=' + encodeURIComponent('  a  \n\n  b  ') +
    '&wrap=square' +
    '&separator=' + encodeURIComponent(';') +
    '&skip_empty=false' +
    '&trim=true' +
    '&last_line_separator=true';
  await page.goto('/tools/wrap-lines-in-quotes/' + qs);
  await expect(page.locator('#in-wrap')).toHaveValue('square', { timeout: 15000 });
  await expect(page.locator('#in-skip_empty')).not.toBeChecked();
  await expect(page.locator('#in-trim')).toBeChecked();
  await expect(page.locator('#in-last_line_separator')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('[a];\n[];\n[b];', { timeout: 15000 });
});

test('wrap-lines-in-quotes supports custom mirrored delimiters and escaping', async ({ page }) => {
  await page.goto('/tools/wrap-lines-in-quotes/');
  await page.fill('#in-text', '5" pipe\na\\b');
  await page.selectOption('#in-wrap', 'custom');
  await page.fill('#in-open', '"');
  await page.check('#in-escape');
  await expect(page.locator('#tool-output')).toHaveText('"5\\" pipe"\n"a\\\\b"', { timeout: 15000 });
});

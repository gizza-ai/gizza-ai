import { test, expect } from './fixtures';

test('css-autoprefixer page adds property + value prefixes', async ({ page }) => {
  await page.goto('/tools/css-autoprefixer/');

  await page.fill('#in-css', 'a {\n  user-select: none;\n  display: flex;\n}\n');
  const out = page.locator('#tool-output');
  // Property prefixes (user-select) in cascade order, standard last.
  await expect(out).toContainText('-webkit-user-select: none;', { timeout: 15000 });
  await expect(out).toContainText('-moz-user-select: none;');
  await expect(out).toContainText('-ms-user-select: none;');
  // Value prefixes (display: flex).
  await expect(out).toContainText('display: -webkit-box;');
  await expect(out).toContainText('display: -webkit-flex;');
  await expect(out).toContainText('display: -ms-flexbox;');
});

test('css-autoprefixer leaves comments + custom properties untouched', async ({ page }) => {
  await page.goto('/tools/css-autoprefixer/');
  await page.fill('#in-css', ':root { --user-select: none; }');
  await expect(page.locator('#tool-output')).toHaveText(':root { --user-select: none; }', {
    timeout: 15000,
  });
});

test('css-autoprefixer query-param deep-link prefills + computes', async ({ page }) => {
  await page.goto(
    '/tools/css-autoprefixer/?css=' + encodeURIComponent('div { position: sticky; }'),
  );
  await expect(page.locator('#in-css')).toHaveValue('div { position: sticky; }', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('position: -webkit-sticky;', {
    timeout: 15000,
  });
});

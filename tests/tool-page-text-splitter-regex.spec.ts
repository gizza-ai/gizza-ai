import { test, expect } from './fixtures';

const tool = '/tools/text-splitter-regex/';

test('text-splitter-regex page splits on whitespace regex', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', 'alpha   beta\t\tgamma');
  await page.fill('#in-pattern', '\\s+');
  await expect(page.locator('#tool-output')).toHaveText('alpha\nbeta\ngamma', {
    timeout: 15000,
  });
});

test('text-splitter-regex renders table output with field pattern', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', 'host: web1\nregion: eu-west');
  await page.fill('#in-pattern', '\\n');
  await page.fill('#in-field_pattern', '\\s*:\\s*');
  await page.selectOption('#in-output', 'csv');
  await expect(page.locator('#tool-output')).toHaveText('host,web1\nregion,eu-west', {
    timeout: 15000,
  });
});

test('text-splitter-regex query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    tool +
      '?text=' +
      encodeURIComponent('one; two;three') +
      '&pattern=' +
      encodeURIComponent('\\s*;\\s*') +
      '&trim=true&remove_empty=true',
  );
  await expect(page.locator('#in-text')).toHaveValue('one; two;three', { timeout: 15000 });
  await expect(page.locator('#in-pattern')).toHaveValue('\\s*;\\s*');
  await expect(page.locator('#in-trim')).toBeChecked();
  await expect(page.locator('#in-remove_empty')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('one\ntwo\nthree', { timeout: 15000 });
});

test('text-splitter-regex max_splits keeps the remainder intact', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', 'ERROR: disk full: /dev/sda1');
  await page.fill('#in-pattern', ':\\s*');
  await page.fill('#in-max_splits', '1');
  await expect(page.locator('#tool-output')).toHaveText('ERROR\ndisk full: /dev/sda1', {
    timeout: 15000,
  });
});

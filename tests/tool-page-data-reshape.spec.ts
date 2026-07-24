import { test, expect } from './fixtures';

test.beforeEach(async ({ page }) => {
  page.on('pageerror', err => console.log('PAGEERROR', err.stack || err.message));
  page.on('console', msg => console.log('BROWSER', msg.type(), msg.text()));
});

test('data-reshape page aggregates CSV into a JSON object', async ({ page }) => {
  await page.goto('/tools/data-reshape/');
  await page.fill('#in-data', 'name,price\napple,2\nbanana,3\ncherry,5');
  await page.fill('#in-query', '{ "total": $sum($.price), "items": $count($) }');
  await page.selectOption('#in-input_format', 'csv');
  await page.selectOption('#in-output_format', 'json');
  await page.check('#in-pretty');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"total": 10', { timeout: 15000 });
  await expect(out).toContainText('"items": 3');
});

test('data-reshape page deep-link filters rows and supports compact output', async ({ page }) => {
  const qs =
    '?data=' + encodeURIComponent('name,price\napple,2\nbanana,20\ncherry,30') +
    '&query=' + encodeURIComponent('$[price > 10].name') +
    '&input_format=csv&output_format=json&pretty=false';
  await page.goto('/tools/data-reshape/' + qs);

  await expect(page.locator('#in-input_format')).toHaveValue('csv', { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('["banana","cherry"]', { timeout: 15000 });
});

test('data-reshape page converts YAML projection to YAML output', async ({ page }) => {
  await page.goto('/tools/data-reshape/');
  await page.fill('#in-data', 'user:\n  first: Ada\n  last: Lovelace');
  await page.fill('#in-query', '{ "name": user.first & " " & user.last }');
  await page.selectOption('#in-input_format', 'yaml');
  await page.selectOption('#in-output_format', 'yaml');

  await expect(page.locator('#tool-output')).toHaveText('name: Ada Lovelace', { timeout: 15000 });
});

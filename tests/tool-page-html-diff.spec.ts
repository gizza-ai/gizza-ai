import { test, expect } from './fixtures';

test('html-diff renders exact line diff output', async ({ page }) => {
  await page.goto('/tools/html-diff/');
  await page.fill('#in-left', '<p>alpha</p><p>beta</p><p>gamma</p>');
  await page.fill('#in-right', '<p>alpha</p><p>BETA</p><p>gamma</p>');
  await page.selectOption('#in-granularity', 'line');
  await page.selectOption('#in-format', 'unified');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('--- left', { timeout: 15000 });
  await expect(out).toContainText('+++ right');
  await expect(out).toContainText('-beta');
  await expect(out).toContainText('+BETA');
});

test('html-diff renders word-level inline markers', async ({ page }) => {
  await page.goto('/tools/html-diff/');
  await page.fill('#in-left', '<p>the quick brown fox</p>');
  await page.fill('#in-right', '<div><span>the slow brown fox</span></div>');
  await page.selectOption('#in-granularity', 'word');
  await page.selectOption('#in-format', 'unified');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('[-quick-]', { timeout: 15000 });
  await expect(out).toContainText('{+slow+}');
  await expect(out).toContainText('brown fox');
});

test('html-diff supports deep-linked JSON and ignore case', async ({ page }) => {
  const params = new URLSearchParams({
    left: '<h1>Pricing</h1><p>Starter plan</p>',
    right: '<div><h1>PRICING</h1><p>Starter plan</p></div>',
    granularity: 'word',
    format: 'json',
    ignore_case: 'true',
    ignore_whitespace: 'false',
    context: '3',
  });
  await page.goto(`/tools/html-diff/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"equal": true', { timeout: 15000 });
  await expect(out).toContainText('"granularity": "word"');
});

import { test, expect } from './fixtures';

test('render-template page renders variables', async ({ page }) => {
  await page.goto('/tools/render-template/');

  await page.fill('#in-template', 'Hello {{name}}!');
  await page.fill('#in-data', '{"name":"Ada"}');
  await expect(page.locator('#tool-output')).toHaveText('Hello Ada!', {
    timeout: 15000,
  });
});

test('render-template page handles each loop and conditional', async ({ page }) => {
  await page.goto('/tools/render-template/');

  await page.fill(
    '#in-template',
    '{{#each items}}- {{this}}\n{{/each}}{{#if admin}}ADMIN{{/if}}',
  );
  await page.fill('#in-data', '{"items":["a","b"],"admin":true}');
  await expect(page.locator('#tool-output')).toHaveText('- a\n- b\nADMIN', {
    timeout: 15000,
  });
});

test('render-template page strict mode errors on missing variable', async ({ page }) => {
  await page.goto('/tools/render-template/');

  await page.fill('#in-template', '[{{missing}}]');
  await page.fill('#in-data', '{}');
  await page.check('#in-strict');
  await expect(page.locator('#tool-output')).toContainText('strict mode', {
    timeout: 15000,
  });
});

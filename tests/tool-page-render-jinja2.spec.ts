import { test, expect } from './fixtures';

test('render-jinja2 page renders variables', async ({ page }) => {
  await page.goto('/tools/render-jinja2/');

  await page.fill('#in-template', 'Hello {{ name }}!');
  await page.fill('#in-data', '{"name":"Ada"}');
  await expect(page.locator('#tool-output')).toHaveText('Hello Ada!', {
    timeout: 15000,
  });
});

test('render-jinja2 page handles for loop and conditional', async ({ page }) => {
  await page.goto('/tools/render-jinja2/');

  await page.fill(
    '#in-template',
    '{% for item in items %}- {{ item }}\n{% endfor %}{% if admin %}ADMIN{% endif %}',
  );
  await page.fill('#in-data', '{"items":["a","b"],"admin":true}');
  // toHaveText normalizes whitespace, so the trailing newline collapses.
  await expect(page.locator('#tool-output')).toHaveText('- a\n- b\nADMIN', {
    timeout: 15000,
  });
});

test('render-jinja2 page renders YAML data with a filter', async ({ page }) => {
  await page.goto('/tools/render-jinja2/');

  await page.fill('#in-template', '{{ name | upper }}');
  await page.fill('#in-data', 'name: ada');
  await page.selectOption('#in-data_format', 'yaml');
  await expect(page.locator('#tool-output')).toHaveText('ADA', {
    timeout: 15000,
  });
});

test('render-jinja2 page strict mode errors on missing variable', async ({ page }) => {
  await page.goto('/tools/render-jinja2/');

  await page.fill('#in-template', '[{{ missing }}]');
  await page.fill('#in-data', '{}');
  await page.check('#in-strict');
  await expect(page.locator('#tool-output')).toContainText('undefined', {
    timeout: 15000,
  });
});

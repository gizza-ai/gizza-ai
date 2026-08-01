import { test, expect } from './fixtures';

// /tools/yaml-formatter/ normalizes YAML in-browser. Field ids are in-<name>:
// #in-yaml is multiline, #in-indent is numeric, and #in-style/#in-sort_keys are
// descriptor-backed selects.

test('yaml-formatter page: block style reindents inline collections', async ({ page }) => {
  await page.goto('/tools/yaml-formatter/');
  await page.fill('#in-yaml', 'name:   gizza\ntags: [a, b]');
  await expect(page.locator('#tool-output')).toHaveText('name: gizza\ntags:\n  - a\n  - b', {
    timeout: 15000,
  });
});

test('yaml-formatter page: 4-space indent is applied to nested block output', async ({ page }) => {
  await page.goto('/tools/yaml-formatter/');
  await page.fill('#in-yaml', 'server:\n  ports: [80, 443]');
  await page.fill('#in-indent', '4');
  await expect(page.locator('#tool-output')).toHaveText('server:\n    ports:\n      - 80\n      - 443', {
    timeout: 15000,
  });
});

test('yaml-formatter page: flow style with ascending key sort', async ({ page }) => {
  await page.goto('/tools/yaml-formatter/');
  await page.fill('#in-yaml', 'b: 1\na: 2');
  await page.selectOption('#in-style', 'flow');
  await page.selectOption('#in-sort_keys', 'asc');
  await expect(page.locator('#tool-output')).toHaveText('{a: 2, b: 1}', {
    timeout: 15000,
  });
});

test('yaml-formatter page: descending key sort', async ({ page }) => {
  await page.goto('/tools/yaml-formatter/');
  await page.fill('#in-yaml', 'a: 1\nb: 2');
  await page.selectOption('#in-sort_keys', 'desc');
  await expect(page.locator('#tool-output')).toHaveText('b: 2\na: 1', {
    timeout: 15000,
  });
});

test('yaml-formatter page: parse errors are surfaced', async ({ page }) => {
  await page.goto('/tools/yaml-formatter/');
  await page.fill('#in-yaml', 'a: [1, 2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('invalid YAML', { timeout: 15000 });
  await expect(out).toHaveClass(/error/);
});

test('yaml-formatter page: query-param deep-link prefills and formats', async ({ page }) => {
  await page.goto(
    '/tools/yaml-formatter/?yaml=' +
      encodeURIComponent('b: 1\na: 2') +
      '&style=flow&sort_keys=asc&indent=2',
  );
  await expect(page.locator('#in-yaml')).toHaveValue('b: 1\na: 2', { timeout: 15000 });
  await expect(page.locator('#in-style')).toHaveValue('flow');
  await expect(page.locator('#in-sort_keys')).toHaveValue('asc');
  await expect(page.locator('#tool-output')).toHaveText('{a: 2, b: 1}', {
    timeout: 15000,
  });
});

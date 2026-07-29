import { test, expect } from './fixtures';

test('query-string-codec parses repeated keys and bracket notation by default', async ({ page }) => {
  await page.goto('/tools/query-string-codec/');
  await page.fill('#in-input', 'name=John+Doe&color=red&color=blue&user[age]=30');
  await expect(page.locator('#tool-output')).toHaveText(
    '{\n  "name": "John Doe",\n  "color": [\n    "red",\n    "blue"\n  ],\n  "user": {\n    "age": "30"\n  }\n}',
    { timeout: 15000 },
  );
});

test('query-string-codec builds sorted query strings from a deep link', async ({ page }) => {
  const qs =
    '?direction=build' +
    '&input=' + encodeURIComponent('{"b":2,"a":1}') +
    '&sort_keys=true' +
    '&prefix_question_mark=true';
  await page.goto('/tools/query-string-codec/' + qs);
  await expect(page.locator('#in-direction')).toHaveValue('build', { timeout: 15000 });
  await expect(page.locator('#in-sort_keys')).toBeChecked();
  await expect(page.locator('#in-prefix_question_mark')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('?a=1&b=2', { timeout: 15000 });
});

test('query-string-codec supports repeat arrays and percent-20 spaces', async ({ page }) => {
  await page.goto('/tools/query-string-codec/');
  await page.selectOption('#in-direction', 'build');
  await page.fill('#in-input', '{"name":"John Doe","tags":["x","y"]}');
  await page.selectOption('#in-array_style', 'repeat');
  await page.uncheck('#in-space_as_plus');
  await expect(page.locator('#tool-output')).toHaveText(
    'name=John%20Doe&tags=x&tags=y',
    { timeout: 15000 },
  );
});

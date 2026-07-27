import { test, expect } from './fixtures';

// /tools/json-sort/ recursively sorts JSON object keys in-browser (pure wasm).
test('json-sort sorts nested keys ascending with 2-space indent', async ({ page }) => {
  await page.goto('/tools/json-sort/');
  await page.fill('#in-json', '{"b":1,"a":{"z":2,"y":3}}');
  await page.selectOption('#in-order', 'asc');
  await page.fill('#in-indent', '2');
  await expect(page.locator('#tool-output')).toHaveText(
    '{\n  "a": {\n    "y": 3,\n    "z": 2\n  },\n  "b": 1\n}',
    { timeout: 15000 },
  );
});

test('json-sort minifies and sorts array elements with indent 0', async ({ page }) => {
  await page.goto('/tools/json-sort/');
  await page.fill('#in-json', '{"nums":[3,1,2],"strs":["b","a"]}');
  await page.selectOption('#in-order', 'asc');
  await page.check('#in-sort_arrays');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('{"nums":[1,2,3],"strs":["a","b"]}', { timeout: 15000 });
});

test('json-sort reports invalid JSON', async ({ page }) => {
  await page.goto('/tools/json-sort/');
  await page.fill('#in-json', '{bad}');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
});

test('json-sort query-param deep-link (descending, case-insensitive)', async ({ page }) => {
  await page.goto(
    '/tools/json-sort/?json=' +
      encodeURIComponent('{"banana":1,"Apple":2,"cherry":3}') +
      '&order=desc&sort_arrays=false&case_insensitive=true&indent=0',
  );
  await expect(page.locator('#in-json')).toHaveValue('{"banana":1,"Apple":2,"cherry":3}', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('{"cherry":3,"banana":1,"Apple":2}', { timeout: 15000 });
});

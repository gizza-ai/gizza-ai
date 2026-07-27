import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('python-dict-to-json page: converts Python booleans nulls tuples and strings', async ({ page }) => {
  await page.goto('/tools/python-dict-to-json/');
  await page.fill(
    '#in-input',
    "{'name': 'Ann', 'active': True, 'scores': (9, 8, 10), 'city': None}",
  );
  await expect(page.locator('#tool-output')).toContainText('"active": true', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    '{\n' +
      '  "name": "Ann",\n' +
      '  "active": true,\n' +
      '  "scores": [\n' +
      '    9,\n' +
      '    8,\n' +
      '    10\n' +
      '  ],\n' +
      '  "city": null\n' +
      '}',
  );
});

test('python-dict-to-json page: minify and sort_keys produce exact compact output', async ({ page }) => {
  await page.goto('/tools/python-dict-to-json/');
  await page.fill('#in-input', "{'b': 1, 'a': 2, 'nested': {'z': False, 'm': None}}");
  await page.selectOption('#in-indent', 'minify');
  await page.check('#in-sort_keys');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"a":2,"b":1,"nested":{"m":null,"z":false}}',
    { timeout: 15000 },
  );
});

test('python-dict-to-json page: ensure_ascii checkbox escapes non ASCII', async ({ page }) => {
  await page.goto('/tools/python-dict-to-json/');
  await page.fill('#in-input', "{'city': 'Zürich', 'emoji': '😀'}");
  await page.selectOption('#in-indent', 'minify');
  await page.check('#in-ensure_ascii');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"city":"Z\\u00fcrich","emoji":"\\ud83d\\ude00"}',
    { timeout: 15000 },
  );
});

test('python-dict-to-json page: query-param deep-link prefills and converts', async ({ page }) => {
  const input = "[\n  'a',  # first\n  'b',\n]";
  await page.goto(
    '/tools/python-dict-to-json/?input=' +
      encodeURIComponent(input) +
      '&indent=minify&sort_keys=false&ensure_ascii=false',
  );
  await expect(page.locator('#in-input')).toHaveValue(input, { timeout: 15000 });
  await expect(page.locator('#in-indent')).toHaveValue('minify');
  await expect(page.locator('#tool-output')).toHaveText('["a","b"]', { timeout: 15000 });
});

test('python-dict-to-json page: nesting cap boundary succeeds and one beyond errors', async ({ page }) => {
  await page.goto('/tools/python-dict-to-json/');
  const ok = '['.repeat(200) + '1' + ']'.repeat(200);
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, ok);
  await page.selectOption('#in-indent', 'minify');
  await expect(page.locator('#tool-output')).toContainText('[[[[', { timeout: 15000 });

  const tooDeep = '['.repeat(201) + '1' + ']'.repeat(201);
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, tooDeep);
  await expect(page.locator('#tool-output.error')).toContainText('nesting deeper than 200', { timeout: 15000 });
});

import { test, expect } from './fixtures';

async function setField(
  page: import('@playwright/test').Page,
  selector: string,
  value: string,
) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('sort-json-array sorts by one numeric key and minifies with indent 0', async ({ page }) => {
  await page.goto('/tools/sort-json-array/');
  await setField(page, '#in-json', '[{"name":"Ada","age":36},{"name":"Bo","age":24},{"name":"Cy","age":41}]');
  await page.fill('#in-keys', 'age');
  await page.selectOption('#in-order', 'asc');
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toHaveText(
    '[{"name":"Bo","age":24},{"name":"Ada","age":36},{"name":"Cy","age":41}]',
    { timeout: 15000 },
  );
});

test('sort-json-array supports multi-key descending, nested paths, missing first, case-insensitive, and indent cap', async ({ page }) => {
  await page.goto('/tools/sort-json-array/');
  await setField(
    page,
    '#in-json',
    '[{"dept":"Eng","salary":120,"user":{"name":"bo"}},' +
      '{"dept":"Eng","salary":150,"user":{"name":"Banana"}},' +
      '{"dept":"Eng","salary":150},' +
      '{"dept":"Eng","salary":150,"user":{"name":"apple"}}]',
  );
  await page.fill('#in-keys', 'dept,-salary,user.name');
  await page.selectOption('#in-order', 'asc');
  await page.selectOption('#in-missing', 'first');
  await page.check('#in-case_insensitive');
  await page.fill('#in-indent', '8');

  const expected = [
    '[',
    '        {',
    '                "dept": "Eng",',
    '                "salary": 150',
    '        },',
    '        {',
    '                "dept": "Eng",',
    '                "salary": 150,',
    '                "user": {',
    '                        "name": "apple"',
    '                }',
    '        },',
    '        {',
    '                "dept": "Eng",',
    '                "salary": 150,',
    '                "user": {',
    '                        "name": "Banana"',
    '                }',
    '        },',
    '        {',
    '                "dept": "Eng",',
    '                "salary": 120,',
    '                "user": {',
    '                        "name": "bo"',
    '                }',
    '        }',
    ']',
  ].join('\n');
  await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15000 });
});

test('sort-json-array deep-link pre-fills and auto-runs descending with missing last', async ({ page }) => {
  const qs = new URLSearchParams({
    json: '[{"n":2},{"x":1},{"n":1}]',
    keys: 'n',
    order: 'desc',
    missing: 'last',
    case_insensitive: 'false',
    indent: '0',
  });
  await page.goto(`/tools/sort-json-array/?${qs.toString()}`);

  await expect(page.locator('#in-json')).toHaveValue('[{"n":2},{"x":1},{"n":1}]', { timeout: 15000 });
  await expect(page.locator('#in-keys')).toHaveValue('n');
  await expect(page.locator('#in-order')).toHaveValue('desc');
  await expect(page.locator('#in-missing')).toHaveValue('last');
  await expect(page.locator('#in-case_insensitive')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('[{"n":2},{"n":1},{"x":1}]', { timeout: 15000 });
});

test('sort-json-array reports non-array JSON errors', async ({ page }) => {
  await page.goto('/tools/sort-json-array/');
  await setField(page, '#in-json', '{"n":1}');
  await page.fill('#in-keys', 'n');
  await expect(page.locator('#tool-output')).toContainText('input must be a JSON array', { timeout: 15000 });
});

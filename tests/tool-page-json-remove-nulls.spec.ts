import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('json-remove-nulls page removes nested nulls and compacts arrays', async ({ page }) => {
  await page.goto('/tools/json-remove-nulls/');
  await page.fill('#in-json', '{"a":1,"b":null,"c":{"d":null,"e":"x"},"f":[1,null,2]}');
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toContainText('"f":[1,2]', { timeout: 15_000 });
  expect(await output(page)).toBe('{"a":1,"c":{"e":"x"},"f":[1,2]}');
});

test('json-remove-nulls deep link can keep arrays and remove empty objects', async ({ page }) => {
  const qs = new URLSearchParams({
    json: '{"a":{"b":null},"rows":[1,null,{"x":null,"y":2}]}',
    arrays: 'keep',
    remove_empty_objects: 'true',
    indent: '0',
  });
  await page.goto(`/tools/json-remove-nulls/?${qs.toString()}`);

  await expect(page.locator('#in-arrays')).toHaveValue('keep', { timeout: 15_000 });
  await expect(page.locator('#in-remove_empty_objects')).toBeChecked();
  expect(await output(page)).toBe('{"rows":[1,null,{"y":2}]}');
});

test('json-remove-nulls reports invalid JSON', async ({ page }) => {
  await page.goto('/tools/json-remove-nulls/');
  await page.fill('#in-json', '{bad}');

  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15_000 });
});

import { test, expect } from './fixtures';

// /tools/json-key-case-convert/ rewrites every JSON object key into one naming
// convention in-browser (pure wasm). Values must stay byte-identical.

test('json-key-case-convert converts nested keys to camelCase', async ({ page }) => {
  await page.goto('/tools/json-key-case-convert/');
  await page.fill('#in-json', '{"user_id":1,"profile_data":{"first_name":"ada","home-address":{"zip_code":"94107"}}}');
  await page.selectOption('#in-target_case', 'camel');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"userId":1,"profileData":{"firstName":"ada","homeAddress":{"zipCode":"94107"}}}',
    { timeout: 15000 },
  );
});

// Advertised-values matrix: one real run per enum choice.
const CASES: Array<[string, string, string]> = [
  ['camel', '{"first_name":"ada"}', '{"firstName":"ada"}'],
  ['pascal', '{"first_name":"ada"}', '{"FirstName":"ada"}'],
  ['snake', '{"lineItems":[{"itemId":1}]}', '{"line_items":[{"item_id":1}]}'],
  ['kebab', '{"userID":7,"HTTPResponse":{"statusCode":200}}', '{"user-id":7,"http-response":{"status-code":200}}'],
  ['constant', '{"first_name":"ada"}', '{"FIRST_NAME":"ada"}'],
];

for (const [target, input, expected] of CASES) {
  test(`json-key-case-convert target_case=${target}`, async ({ page }) => {
    await page.goto('/tools/json-key-case-convert/');
    await page.fill('#in-json', input);
    await page.selectOption('#in-target_case', target);
    await page.fill('#in-indent', '0');
    await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15000 });
  });
}

test('json-key-case-convert honours indent 2 (pretty output)', async ({ page }) => {
  await page.goto('/tools/json-key-case-convert/');
  await page.fill('#in-json', '{"a_b":1}');
  await page.fill('#in-indent', '2');
  await expect(page.locator('#tool-output')).toContainText('"aB"', { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe('{\n  "aB": 1\n}');
});

test('json-key-case-convert indent boundary 8', async ({ page }) => {
  await page.goto('/tools/json-key-case-convert/');
  await page.fill('#in-json', '{"a_b":1}');
  await page.fill('#in-indent', '8');
  await expect(page.locator('#tool-output')).toContainText('"aB"', { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe('{\n        "aB": 1\n}');
});

// Non-default checkbox state: recursion off renames only the outermost object.
test('json-key-case-convert recurse off renames only the top level', async ({ page }) => {
  await page.goto('/tools/json-key-case-convert/');
  await page.fill('#in-json', '{"outer_key":{"inner_key":1}}');
  await page.fill('#in-indent', '0');
  await page.uncheck('#in-recurse');
  await expect(page.locator('#tool-output')).toHaveText('{"outerKey":{"inner_key":1}}', { timeout: 15000 });
});

// Non-default checkbox state: sigils stripped when preserve_prefix is off.
test('json-key-case-convert preserve_prefix default keeps _id, off strips it', async ({ page }) => {
  await page.goto('/tools/json-key-case-convert/');
  await page.fill('#in-json', '{"_id":1,"$schema_url":"x"}');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('{"_id":1,"$schemaUrl":"x"}', { timeout: 15000 });
  await page.uncheck('#in-preserve_prefix');
  await expect(page.locator('#tool-output')).toHaveText('{"id":1,"schemaUrl":"x"}', { timeout: 15000 });
});

test('json-key-case-convert preserve_keys skips exact names', async ({ page }) => {
  await page.goto('/tools/json-key-case-convert/');
  await page.fill('#in-json', '{"Content-Type":"a","userName":"b"}');
  await page.selectOption('#in-target_case', 'snake');
  await page.fill('#in-preserve_keys', 'Content-Type');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('{"Content-Type":"a","user_name":"b"}', { timeout: 15000 });
});

test('json-key-case-convert reports invalid JSON', async ({ page }) => {
  await page.goto('/tools/json-key-case-convert/');
  await page.fill('#in-json', '{bad}');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
});

test('json-key-case-convert refuses a key collision', async ({ page }) => {
  await page.goto('/tools/json-key-case-convert/');
  await page.fill('#in-json', '{"user_name":1,"userName":2}');
  await expect(page.locator('#tool-output')).toContainText('key collision at $', { timeout: 15000 });
});

test('json-key-case-convert query-param deep-link (snake, minified)', async ({ page }) => {
  await page.goto(
    '/tools/json-key-case-convert/?json=' +
      encodeURIComponent('{"lineItems":[{"itemId":1,"unitPrice":9.5}]}') +
      '&target_case=snake&recurse=true&preserve_keys=&preserve_prefix=true&indent=0',
  );
  await expect(page.locator('#in-json')).toHaveValue('{"lineItems":[{"itemId":1,"unitPrice":9.5}]}', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('{"line_items":[{"item_id":1,"unit_price":9.5}]}', {
    timeout: 15000,
  });
});

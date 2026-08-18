import { test, expect } from './fixtures';

// /tools/json-dedupe-array/ removes duplicate elements from a JSON array in-browser.

test('json-dedupe-array removes duplicate whole elements', async ({ page }) => {
  await page.goto('/tools/json-dedupe-array/');
  await page.fill('#in-json', '[{"id":1,"email":"ada@x.com"},{"id":2,"email":"bo@x.com"},{"id":1,"email":"ada@x.com"}]');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('[{"id":1,"email":"ada@x.com"},{"id":2,"email":"bo@x.com"}]', {
    timeout: 15000,
  });
});

test('json-dedupe-array ignores object key order when comparing', async ({ page }) => {
  await page.goto('/tools/json-dedupe-array/');
  await page.fill('#in-json', '[{"b":2,"a":1},{"a":1,"b":2}]');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('[{"b":2,"a":1}]', { timeout: 15000 });
});

const KEEP_CASES: Array<[string, string]> = [
  ['first', '[{"id":1,"n":"a"},{"id":2,"n":"b"}]'],
  ['last', '[{"id":2,"n":"b"},{"id":1,"n":"z"}]'],
];

for (const [keep, expected] of KEEP_CASES) {
  test(`json-dedupe-array keep=${keep}`, async ({ page }) => {
    await page.goto('/tools/json-dedupe-array/');
    await page.fill('#in-json', '[{"id":1,"n":"a"},{"id":2,"n":"b"},{"id":1,"n":"z"}]');
    await page.fill('#in-keys', 'id');
    await page.selectOption('#in-keep', keep);
    await page.fill('#in-indent', '0');
    await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15000 });
  });
}

const OUTPUT_CASES: Array<[string, string]> = [
  ['unique', '[1,2]'],
  ['duplicates', '[1,1]'],
  ['report', '{"total":4,"unique":2,"removed":2,"duplicate_groups":[{"count":3,"indexes":[0,2,3],"kept_index":0,"value":1}]}'],
];

for (const [output, expected] of OUTPUT_CASES) {
  test(`json-dedupe-array output=${output}`, async ({ page }) => {
    await page.goto('/tools/json-dedupe-array/');
    await page.fill('#in-json', '[1,2,1,1]');
    await page.selectOption('#in-output', output);
    await page.fill('#in-indent', '0');
    await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15000 });
  });
}

test('json-dedupe-array dedupes by nested key path', async ({ page }) => {
  await page.goto('/tools/json-dedupe-array/');
  await page.fill('#in-json', '[{"user":{"email":"a@x.com"},"v":1},{"user":{"email":"a@x.com"},"v":2},{"user":{"email":"b@x.com"},"v":3}]');
  await page.fill('#in-keys', 'user.email');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('[{"user":{"email":"a@x.com"},"v":1},{"user":{"email":"b@x.com"},"v":3}]', {
    timeout: 15000,
  });
});

test('json-dedupe-array ignore_case checkbox folds values and field names', async ({ page }) => {
  await page.goto('/tools/json-dedupe-array/');
  await page.fill('#in-json', '[{"ID":"A"},{"id":"a"},{"Id":"B"}]');
  await page.fill('#in-keys', 'id');
  await page.check('#in-ignore_case');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('[{"ID":"A"},{"Id":"B"}]', { timeout: 15000 });
});

test('json-dedupe-array root path keeps the wrapper', async ({ page }) => {
  await page.goto('/tools/json-dedupe-array/');
  await page.fill('#in-json', '{"ok":true,"data":{"items":[{"sku":"A1"},{"sku":"B2"},{"sku":"A1"}]}}');
  await page.fill('#in-root', 'data.items');
  await page.fill('#in-keys', 'sku');
  await page.fill('#in-indent', '0');
  await expect(page.locator('#tool-output')).toHaveText('{"ok":true,"data":{"items":[{"sku":"A1"},{"sku":"B2"}]}}', {
    timeout: 15000,
  });
});

test('json-dedupe-array indent boundary 8', async ({ page }) => {
  await page.goto('/tools/json-dedupe-array/');
  await page.fill('#in-json', '[1,1]');
  await page.fill('#in-indent', '8');
  await expect(page.locator('#tool-output')).toContainText('1', { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe('[\n        1\n]');
});

test('json-dedupe-array reports invalid JSON', async ({ page }) => {
  await page.goto('/tools/json-dedupe-array/');
  await page.fill('#in-json', '[1, 2,]');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
});

test('json-dedupe-array rejects non-array top level with root hint', async ({ page }) => {
  await page.goto('/tools/json-dedupe-array/');
  await page.fill('#in-json', '{"items":[1,1]}');
  await expect(page.locator('#tool-output')).toContainText('expected a JSON array', { timeout: 15000 });
});

test('json-dedupe-array query-param deep-link', async ({ page }) => {
  await page.goto(
    '/tools/json-dedupe-array/?json=' +
      encodeURIComponent('{"data":{"items":[{"sku":"A1"},{"sku":"B2"},{"sku":"A1"}]}}') +
      '&keys=sku&root=data.items&keep=first&ignore_case=false&output=unique&indent=0',
  );
  await expect(page.locator('#in-root')).toHaveValue('data.items', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('{"data":{"items":[{"sku":"A1"},{"sku":"B2"}]}}', {
    timeout: 15000,
  });
});

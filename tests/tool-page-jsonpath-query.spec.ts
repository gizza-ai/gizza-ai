import { test, expect } from './fixtures';

// /tools/jsonpath-query/ evaluates a JSONPath (RFC 9535) expression over JSON in-browser.
test('jsonpath-query page selects matched nodes', async ({ page }) => {
  await page.goto('/tools/jsonpath-query/');
  await page.fill('#in-path', '$.store.book[*].author');
  await page.fill('#in-json', '{"store":{"book":[{"author":"Nigel Rees"},{"author":"Evelyn Waugh"}]}}');
  await expect(page.locator('#tool-output')).toContainText('Nigel Rees', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Evelyn Waugh');
});

test('jsonpath-query page deep-link with filter selector', async ({ page }) => {
  const qs =
    '?path=' + encodeURIComponent('$.book[?(@.price < 10)].title') +
    '&json=' + encodeURIComponent('{"book":[{"title":"Cheap","price":5},{"title":"Pricey","price":99}]}');
  await page.goto('/tools/jsonpath-query/' + qs);
  await expect(page.locator('#in-path')).toHaveValue('$.book[?(@.price < 10)].title', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Cheap', { timeout: 15000 });
  await expect(page.locator('#tool-output')).not.toContainText('Pricey');
});

test('jsonpath-query page wrap returns a JSON array', async ({ page }) => {
  await page.goto('/tools/jsonpath-query/');
  await page.fill('#in-path', '$..price');
  await page.fill('#in-json', '{"a":{"price":1},"b":{"price":2}}');
  await page.check('#in-wrap');
  await expect(page.locator('#tool-output')).toContainText('[1,2]', { timeout: 15000 });
});

import { test, expect } from './fixtures';

const doc = '{"store":{"book":[{"title":"A","price":5},{"title":"B","price":12}],"open":true}}';

test('json-path-edit page gets, sets, deletes, and pretty-prints JSON paths', async ({ page }) => {
  await page.goto('/tools/json-path-edit/');

  await page.fill('#in-json', doc);
  await page.fill('#in-path', 'store.book[1].title');
  await page.selectOption('#in-operation', 'get');
  await page.uncheck('#in-pretty');
  await expect(page.locator('#tool-output')).toHaveText('"B"', { timeout: 15000 });

  await page.fill('#in-path', 'store.book[0].price');
  await page.selectOption('#in-operation', 'set');
  await page.fill('#in-value', '9');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"store":{"book":[{"title":"A","price":9},{"title":"B","price":12}],"open":true}}',
    { timeout: 15000 },
  );

  await page.fill('#in-path', 'store.open');
  await page.selectOption('#in-operation', 'delete');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"store":{"book":[{"title":"A","price":5},{"title":"B","price":12}]}}',
    { timeout: 15000 },
  );

  await page.selectOption('#in-operation', 'set');
  await page.fill('#in-path', 'store.note');
  await page.fill('#in-value', 'hello');
  await page.check('#in-pretty');
  await expect(page.locator('#tool-output')).toContainText('  "note": "hello"', {
    timeout: 15000,
  });
});

test('json-path-edit supports deep-link params and quoted keys', async ({ page }) => {
  const qs = new URLSearchParams({
    json: '{"a.b":{"c":1}}',
    path: '["a.b"].c',
    operation: 'get',
    pretty: 'false',
  });

  await page.goto('/tools/json-path-edit/?' + qs.toString());

  await expect(page.locator('#in-path')).toHaveValue('["a.b"].c', { timeout: 15000 });
  await expect(page.locator('#in-operation')).toHaveValue('get');
  await expect(page.locator('#in-pretty')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('1', { timeout: 15000 });
});

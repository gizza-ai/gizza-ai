import { test, expect } from './fixtures';

async function outputJson(page: any) {
  const text = await page.locator('#tool-output').innerText({ timeout: 15000 });
  return JSON.parse(text);
}

test('json-to-dynamodb-batch converts objects to PutRequest AttributeValues', async ({ page }) => {
  await page.goto('/tools/json-to-dynamodb-batch/');
  await page.fill('#in-json', '[{"id":"user#1","age":36,"active":true,"tags":["a",2],"coupon":null}]');
  await page.fill('#in-table_name', 'Users');

  const out = await outputJson(page);
  const item = out.RequestItems.Users[0].PutRequest.Item;
  expect(item.id).toEqual({ S: 'user#1' });
  expect(item.age).toEqual({ N: '36' });
  expect(item.active).toEqual({ BOOL: true });
  expect(item.tags).toEqual({ L: [{ S: 'a' }, { N: '2' }] });
  expect(item.coupon).toEqual({ NULL: true });
});

test('json-to-dynamodb-batch emits compact DeleteRequest when selected', async ({ page }) => {
  await page.goto('/tools/json-to-dynamodb-batch/');
  await page.fill('#in-json', '[{"id":"user#1"}]');
  await page.fill('#in-table_name', 'Users');
  await page.selectOption('#in-operation', 'delete');
  await page.uncheck('#in-pretty');

  await expect(page.locator('#tool-output')).toHaveText(
    '{"RequestItems":{"Users":[{"DeleteRequest":{"Key":{"id":{"S":"user#1"}}}}]}}',
    { timeout: 15000 },
  );
});

test('json-to-dynamodb-batch deep-link pre-fills params and auto-runs', async ({ page }) => {
  const params = new URLSearchParams({
    json: '[{"id":"user#2","score":9.5}]',
    table_name: 'Scores',
    operation: 'put',
    pretty: 'true',
  });
  await page.goto(`/tools/json-to-dynamodb-batch/?${params.toString()}`);
  await expect(page.locator('#in-operation')).toHaveValue('put');

  const out = await outputJson(page);
  expect(out.RequestItems.Scores[0].PutRequest.Item.score).toEqual({ N: '9.5' });
});

import { test, expect } from './fixtures';

const left = '{"requestId":"req-1","data":{"total":2,"status":"ok"}}';
const right = '{"requestId":"req-2","data":{"total":3,"status":"ok"}}';

test('api-response-diff page ignores volatile fields and reports a real change', async ({ page }) => {
  await page.goto('/tools/api-response-diff/');
  await page.fill('#in-left', left);
  await page.fill('#in-right', right);
  await page.fill('#in-ignore', 'requestId');
  await page.selectOption('#in-output', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText(
    '1 difference (0 added, 0 removed, 1 changed, 0 type changed); 1 ignored\n~ $.data.total: 2 -> 3',
    { timeout: 15_000 }
  );
});

test('api-response-diff deep-link pairs reordered arrays by key', async ({ page }) => {
  const qs = new URLSearchParams({
    left: '{"items":[{"id":"a","price":1},{"id":"b","price":2}]}',
    right: '{"items":[{"id":"b","price":2},{"id":"a","price":4}]}',
    ignore: '',
    ignore_timestamps: 'false',
    ignore_uuids: 'false',
    array_match: 'key',
    array_key: 'id',
    numeric_tolerance: '0',
    ignore_case: 'false',
    trim_strings: 'false',
    null_equals_missing: 'false',
    coerce_types: 'false',
    output: 'summary',
    indent: '2',
  });

  await page.goto(`/tools/api-response-diff/?${qs.toString()}`);
  await expect(page.locator('#in-left')).toHaveValue(qs.get('left')!, { timeout: 15_000 });
  await expect(page.locator('#in-array_match')).toHaveValue('key');
  await expect(page.locator('#in-array_key')).toHaveValue('id');
  await expect(page.locator('#tool-output')).toHaveText(
    '1 difference (0 added, 0 removed, 1 changed, 0 type changed); 0 ignored\n~ $.items[id=a].price: 1 -> 4',
    { timeout: 15_000 }
  );
});

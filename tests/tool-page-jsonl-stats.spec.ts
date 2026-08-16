import { test, expect } from './fixtures';

const SAMPLE = '{"id":1,"status":"ok","latency_ms":12}\n' +
  '{"id":2,"status":"error","latency_ms":940,"err":{"code":"timeout"}}\n' +
  '{"id":3,"status":"ok","latency_ms":31}\n' +
  '{"id":4,"status":"ok"}';

const NESTED = '{"user":{"id":7},"items":[{"sku":"a"},{"sku":"b"}]}\n' +
  '{"user":{"id":8},"items":[{"sku":"a"}]}';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('jsonl-stats page emits exact text coverage stats', async ({ page }) => {
  await page.goto('/tools/jsonl-stats/');
  await setTextarea(page.locator('#in-input'), SAMPLE);
  await page.selectOption('#in-format', 'text');
  await page.selectOption('#in-sort', 'frequency');
  await page.fill('#in-depth', '1');
  await page.fill('#in-samples', '0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('records: 4 · lines read: 4 · invalid: 0', { timeout: 15000 });
  await expect(out).toContainText('keys: 4 (depth 1)');
  await expect(out).toContainText('latency_ms        3       75%  number 3');
  await expect(out).toContainText('status            4      100%  string 4');
});

test('jsonl-stats page supports CSV with nested array paths', async ({ page }) => {
  await page.goto('/tools/jsonl-stats/');
  await setTextarea(page.locator('#in-input'), NESTED);
  await page.fill('#in-depth', '3');
  await page.selectOption('#in-format', 'csv');
  await page.selectOption('#in-sort', 'name');
  await page.fill('#in-samples', '0');
  await page.uncheck('#in-value_stats');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('key,present,coverage,types,distinct', { timeout: 15000 });
  await expect(out).toContainText('items[].sku,2,100%,string 3,2');
  await expect(out).toContainText('user.id,2,100%,number 2,2');
});

test('jsonl-stats deep-link pre-fills non-default params and reports invalid lines as errors', async ({ page }) => {
  const params = new URLSearchParams({
    input: '{"a":1}\nnot-json',
    depth: '2',
    format: 'json',
    sort: 'name',
    max_keys: '1',
    samples: '1',
    value_stats: 'false',
    distinct: 'false',
    invalid: 'skip',
  });
  await page.goto(`/tools/jsonl-stats/?${params.toString()}`);

  await expect(page.locator('#in-depth')).toHaveValue('2');
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#in-sort')).toHaveValue('name');
  await expect(page.locator('#in-max_keys')).toHaveValue('1');
  await expect(page.locator('#in-samples')).toHaveValue('1');
  await expect(page.locator('#in-value_stats')).not.toBeChecked();
  await expect(page.locator('#in-distinct')).not.toBeChecked();
  await expect(page.locator('#in-invalid')).toHaveValue('skip');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"records": 1', { timeout: 15000 });
  await expect(out).toContainText('"invalid_lines": 1');
  await expect(out).toContainText('"keys_shown": 1');
});

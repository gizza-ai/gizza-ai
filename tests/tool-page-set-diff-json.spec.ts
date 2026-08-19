import { test, expect } from './fixtures';

const tool = '/tools/set-diff-json/';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  arrayA: string,
  arrayB: string,
  operation = 'difference',
  key = '',
  caseInsensitive = 'false',
  dedupe = 'true',
  output = 'report',
  indent = '2',
): Promise<string> {
  return await page.evaluate(
    async ({ arrayA, arrayB, operation, key, caseInsensitive, dedupe, output, indent }) => {
      const mod = await import('/tools/set-diff-json/gizza_ai_set_diff_json_web.js');
      await mod.default('/tools/set-diff-json/gizza_ai_set_diff_json_web_bg.wasm');
      return mod.run(arrayA, arrayB, operation, key, caseInsensitive, dedupe, output, indent);
    },
    { arrayA, arrayB, operation, key, caseInsensitive, dedupe, output, indent },
  );
}

test('set-diff-json page reports keyed A minus B with counts', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-array_a'), '[{"id":1,"name":"Ada"},{"id":2,"name":"Bo"},{"id":3,"name":"Cy"}]');
  await setTextarea(page.locator('#in-array_b'), '[{"id":2,"name":"Changed"}]');
  await page.selectOption('#in-operation', 'difference');
  await page.fill('#in-key', 'id');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"operation": "difference"', { timeout: 15_000 });
  await expect(out).toContainText('"matched_by": "id"');
  await expect(out).toContainText('"only_in_a": 2');
  await expect(out).toContainText('"result": 2');
  await expect(out).toContainText('"name": "Ada"');
  await expect(out).toContainText('"name": "Cy"');
  await expect(out).not.toContainText('Changed');
});

test('set-diff-json deep link pre-fills and runs case-insensitive intersection', async ({ page }) => {
  const qs = new URLSearchParams({
    array_a: '["Apple","Pear"]',
    array_b: '["APPLE","Grape"]',
    operation: 'intersection',
    key: '',
    case_insensitive: 'true',
    dedupe: 'true',
    output: 'array',
    indent: '0',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-array_a')).toHaveValue('["Apple","Pear"]', { timeout: 15_000 });
  await expect(page.locator('#in-array_b')).toHaveValue('["APPLE","Grape"]');
  await expect(page.locator('#in-operation')).toHaveValue('intersection');
  await expect(page.locator('#in-case_insensitive')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('array');
  await expect(page.locator('#in-indent')).toHaveValue('0');
  await expect(page.locator('#tool-output')).toContainText('["Apple"]');
});

test('set-diff-json wasm covers operations, dedupe, boundary indent, and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-array_a');

  expect(await runWasm(page, '[1,2]', '[2,3]', 'union', '', 'false', 'true', 'array', '0')).toBe('[1,2,3]');
  expect(await runWasm(page, '[1,2]', '[2,3]', 'intersection', '', 'false', 'true', 'array', '0')).toBe('[2]');
  expect(await runWasm(page, '[1,2]', '[2,3]', 'difference', '', 'false', 'true', 'array', '0')).toBe('[1]');
  expect(await runWasm(page, '[1,2]', '[2,3]', 'symmetric_difference', '', 'false', 'true', 'array', '0')).toBe('[1,3]');
  expect(await runWasm(page, '[1,1,2]', '[2]', 'difference', '', 'false', 'false', 'array', '0')).toBe('[1,1]');
  expect(await runWasm(page, '[1]', '[]', 'difference', '', 'false', 'true', 'array', '8')).toBe('[\n        1\n]');

  await expect(runWasm(page, '{"not":"array"}', '[1]')).rejects.toThrow(/array A is not a JSON array/);
  await expect(runWasm(page, '[{"id":1},{"name":"Bo"}]', '[{"id":1}]', 'difference', 'id')).rejects.toThrow(/array A element 1 has no "id" field/);
});

test('set-diff-json ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Records missing from B (match on id)',
    'Emails in both lists',
    'Merge two tag lists',
    'Rows on exactly one side (by sku)',
    'Keep every occurrence (no dedupe)',
  ]);
});

import { test, expect } from './fixtures';

// Exact-output assertions. Single-line results use toHaveText; multi-line
// results (NDJSON, multi-row CSV/TSV, pretty JSON) are compared against
// #tool-output.textContent so newlines/tabs are asserted exactly (toHaveText
// normalizes whitespace). The page writes `String(value)` into #tool-output
// verbatim, so CSV/TSV keep their trailing newline and NDJSON has none.

test('CSV → JSON with type inference (from=auto, to=json defaults)', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', 'name,age\nAlice,30\nBob,25');
  await expect(page.locator('#tool-output')).toHaveText(
    '[{"name":"Alice","age":30},{"name":"Bob","age":25}]',
    { timeout: 15000 },
  );
});

test('JSON → NDJSON (to=ndjson) — one compact record per line', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', '[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}]');
  await page.selectOption('#in-to', 'ndjson');
  await expect(page.locator('#tool-output')).toHaveText(/"id":1/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    '{"id":1,"name":"Alice"}\n{"id":2,"name":"Bob"}',
  );
});

test('CSV → NDJSON (to=ndjson)', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', 'id,city\n1,NYC\n2,LA');
  await page.selectOption('#in-to', 'ndjson');
  await expect(page.locator('#tool-output')).toHaveText(/NYC/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    '{"id":1,"city":"NYC"}\n{"id":2,"city":"LA"}',
  );
});

test('NDJSON → CSV (from=ndjson, to=csv) — unions keys', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', '{"a":1,"b":2}\n{"a":3,"c":4}');
  await page.selectOption('#in-from', 'ndjson');
  await page.selectOption('#in-to', 'csv');
  await expect(page.locator('#tool-output')).toHaveText(/a,b,c/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe('a,b,c\n1,2,\n3,,4\n');
});

test('TSV → JSON (from=tsv, to=json)', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', 'x\ty\n1\t2');
  await page.selectOption('#in-from', 'tsv');
  await expect(page.locator('#tool-output')).toHaveText('[{"x":1,"y":2}]', {
    timeout: 15000,
  });
});

test('CSV → TSV (to=tsv) — re-delimits with tabs', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', 'a,b\n1,2');
  await page.selectOption('#in-to', 'tsv');
  await expect(page.locator('#tool-output')).toHaveText(/a\s+b/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe('a\tb\n1\t2\n');
});

test('infer types OFF keeps every cell a string (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', 'age,flag\n30,true');
  await page.uncheck('#in-infer_types');
  await expect(page.locator('#tool-output')).toHaveText(
    '[{"age":"30","flag":"true"}]',
    { timeout: 15000 },
  );
});

test('flatten ON expands nested objects to dot-notation columns (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', '[{"name":"Al","addr":{"city":"NYC","zip":"10001"}}]');
  await page.selectOption('#in-to', 'csv');
  await page.check('#in-flatten');
  await expect(page.locator('#tool-output')).toHaveText(/addr\.city/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    'name,addr.city,addr.zip\nAl,NYC,10001\n',
  );
});

test('headers OFF gives arrays-of-arrays (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', 'a,b\nc,d');
  await page.uncheck('#in-headers');
  await expect(page.locator('#tool-output')).toHaveText('[["a","b"],["c","d"]]', {
    timeout: 15000,
  });
});

test('pretty-print indents the JSON target (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/data-format-converter/');
  await page.fill('#in-data', 'x\n1');
  await page.check('#in-pretty');
  await expect(page.locator('#tool-output')).toHaveText(/"x": 1/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    '[\n  {\n    "x": 1\n  }\n]',
  );
});

test('query-param deep-link prefills fields + computes', async ({ page }) => {
  await page.goto(
    '/tools/data-format-converter/?data=' +
      encodeURIComponent('id,name\n1,Alice') +
      '&to=ndjson',
  );
  await expect(page.locator('#in-data')).toHaveValue('id,name\n1,Alice', { timeout: 15000 });
  await expect(page.locator('#in-to')).toHaveValue('ndjson');
  await expect(page.locator('#tool-output')).toHaveText(/Alice/, { timeout: 15000 });
  expect(await page.locator('#tool-output').textContent()).toBe(
    '{"id":1,"name":"Alice"}',
  );
});

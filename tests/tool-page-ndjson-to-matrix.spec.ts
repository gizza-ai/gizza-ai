import { test, expect } from './fixtures';

const tool = '/tools/ndjson-to-matrix/';
const logRecords = '{"id":1,"latency_ms":12,"user":{"tier":"pro"}}\n{"id":2,"latency_ms":940}\n{"id":3,"latency_ms":31,"user":{"tier":"free"}}';

async function runWasm(
  page,
  data: string,
  format = 'csv',
  delimiter = 'comma',
  separator = '.',
  arrays = 'index',
  columns = '',
  columnOrder = 'first-seen',
  fill = '',
  headers = 'true',
  rowIndex = 'false',
  numericOnly = 'false',
  transpose = 'false',
  maxDepth = '0',
  limit = '0',
  invalid = 'error',
): Promise<string> {
  return await page.evaluate(
    async (args) => {
      const mod = await import('/tools/ndjson-to-matrix/gizza_ai_ndjson_to_matrix_web.js');
      await mod.default('/tools/ndjson-to-matrix/gizza_ai_ndjson_to_matrix_web_bg.wasm');
      return mod.run(
        args.data,
        args.format,
        args.delimiter,
        args.separator,
        args.arrays,
        args.columns,
        args.columnOrder,
        args.fill,
        args.headers,
        args.rowIndex,
        args.numericOnly,
        args.transpose,
        args.maxDepth,
        args.limit,
        args.invalid,
      );
    },
    { data, format, delimiter, separator, arrays, columns, columnOrder, fill, headers, rowIndex, numericOnly, transpose, maxDepth, limit, invalid },
  );
}

test('ndjson-to-matrix page aligns ragged records as CSV', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', logRecords);
  await page.selectOption('#in-format', 'csv');
  await page.selectOption('#in-delimiter', 'comma');
  await page.selectOption('#in-arrays', 'index');
  await page.selectOption('#in-column_order', 'first-seen');

  await expect(page.locator('#tool-output')).toHaveText(
    'id,latency_ms,user.tier\n1,12,pro\n2,940,\n3,31,free',
    { timeout: 15000 },
  );
});

test('ndjson-to-matrix deep link pre-fills matrix controls', async ({ page }) => {
  const qs = new URLSearchParams({
    data: '{"x":1,"y":2,"label":"a"}\n{"x":3,"y":4,"label":"b"}\n{"x":5,"label":"c"}',
    format: 'matrix',
    delimiter: 'comma',
    separator: '.',
    arrays: 'index',
    columns: '',
    column_order: 'first-seen',
    fill: '0',
    headers: 'false',
    row_index: 'false',
    numeric_only: 'true',
    transpose: 'false',
    max_depth: '0',
    limit: '0',
    invalid: 'error',
  });
  await page.goto(`${tool}?${qs.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('matrix', { timeout: 15000 });
  await expect(page.locator('#in-fill')).toHaveValue('0');
  await expect(page.locator('#in-headers')).not.toBeChecked();
  await expect(page.locator('#in-numeric_only')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText('1  2\n3  4\n5  0');
});

test('ndjson-to-matrix wasm covers formats and validation errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  expect(await runWasm(page, logRecords, 'json')).toContain('["id", "latency_ms", "user.tier"]');
  expect(await runWasm(page, logRecords, 'tsv', 'comma', '.', 'index', '', 'alpha', 'NA', 'true', 'true', 'false', 'false', '0', '2')).toBe(
    'row\tid\tlatency_ms\tuser.tier\n1\t1\t12\tpro\n2\t2\t940\tNA',
  );
  expect(await runWasm(page, '{"v":[1,2]}', 'csv', 'comma', '.', 'json')).toBe('v\n"[1,2]"');
  expect(await runWasm(page, '{"x":1}\noops\n{"x":2}', 'csv', 'comma', '.', 'index', '', 'first-seen', '', 'true', 'false', 'false', 'false', '0', '0', 'skip')).toBe('x\n1\n2');
  await expect(runWasm(page, '{"x":1}\noops')).rejects.toThrow(/invalid JSON on line 2/);
  await expect(runWasm(page, logRecords, 'yaml')).rejects.toThrow(/expected format/);
});

test('ndjson-to-matrix page ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Ragged log records → CSV',
    'Numeric matrix for numpy (no header, 0-filled)',
    'Vectors indexed into columns → TSV',
    'Transposed series for plotting',
  ]);
});

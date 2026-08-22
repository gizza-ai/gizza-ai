import { test, expect } from './fixtures';

const SAMPLE = 'id,name,score\n1,Alice,9.5\n2,Bob,7';
const CSV_ONLY = 'id,score\n1,9.5\n2,7';

async function runWasm(
  page: any,
  data = SAMPLE,
  delimiter = 'auto',
  header = 'auto',
  output = 'columns',
  nullTokens = 'NA,N/A,NULL,null,None,nan',
  allowBlanks = 'true',
  minNumericRatio = '1',
  normalize = 'true',
): Promise<string> {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/csv-numeric-column-extractor/gizza_ai_csv_numeric_column_extractor_web.js');
    await mod.default('/tools/csv-numeric-column-extractor/gizza_ai_csv_numeric_column_extractor_web_bg.wasm');
    return mod.run(
      args.data,
      args.delimiter,
      args.header,
      args.output,
      args.nullTokens,
      args.allowBlanks,
      args.minNumericRatio,
      args.normalize,
    );
  }, { data, delimiter, header, output, nullTokens, allowBlanks, minNumericRatio, normalize });
}

test('csv-numeric-column-extractor wasm emits typed arrays and skip reasons', async ({ page }) => {
  await page.goto('/tools/csv-numeric-column-extractor/');
  await page.waitForSelector('#in-data');

  const result = JSON.parse(await runWasm(page));
  expect(result).toMatchObject({ delimiter: 'comma', header: true, rows: 2, columns_total: 3, numeric_columns: 2 });
  expect(result.columns[0]).toMatchObject({ name: 'id', type: 'integer', values: [1, 2] });
  expect(result.columns[1]).toMatchObject({ name: 'score', type: 'float', values: [9.5, 7] });
  expect(result.skipped[0]).toMatchObject({ name: 'name', example: 'Alice' });
});

test('csv-numeric-column-extractor wasm covers output modes, delimiters and header modes', async ({ page }) => {
  await page.goto('/tools/csv-numeric-column-extractor/');
  await page.waitForSelector('#in-data');

  await expect(runWasm(page, SAMPLE, 'auto', 'auto', 'csv')).resolves.toBe(CSV_ONLY);
  await expect(runWasm(page, SAMPLE, 'auto', 'auto', 'names')).resolves.toBe('id\nscore');
  expect(JSON.parse(await runWasm(page, SAMPLE, 'auto', 'auto', 'records'))).toEqual([
    { id: 1, score: 9.5 },
    { id: 2, score: 7 },
  ]);

  await expect(runWasm(page, 'a\tb\n1\t2', 'tab', 'present', 'csv')).resolves.toBe('a,b\n1,2');
  await expect(runWasm(page, 'a;b\n1;2', 'semicolon', 'present', 'csv')).resolves.toBe('a,b\n1,2');
  await expect(runWasm(page, 'a|b\n1|2', 'pipe', 'present', 'csv')).resolves.toBe('a,b\n1,2');
  await expect(runWasm(page, '10;20;30\n40;50;60', 'auto', 'absent', 'names')).resolves.toBe('column_1\ncolumn_2\ncolumn_3');
});

test('csv-numeric-column-extractor wasm covers booleans, ratio, nulls and accounting formats', async ({ page }) => {
  await page.goto('/tools/csv-numeric-column-extractor/');
  await page.waitForSelector('#in-data');

  const accounting = 'region,revenue,margin\nEMEA,"$1,234.50",45%\nAPAC,"$2,000",(500)';
  await expect(runWasm(page, accounting, 'auto', 'auto', 'csv')).resolves.toBe('revenue,margin\n1234.5,45\n2000,-500');
  await expect(runWasm(page, accounting, 'auto', 'auto', 'names', 'NA,N/A,NULL,null,None,nan', 'true', '1', 'false'))
    .resolves.toBe('');

  const gappy = 'name,score\nAlice,1\nBob,\nCy,3';
  await expect(runWasm(page, gappy, 'auto', 'auto', 'csv')).resolves.toBe('score\n1\n\n3');
  const strictGappy = JSON.parse(await runWasm(page, gappy, 'auto', 'auto', 'columns', 'NA,N/A,NULL,null,None,nan', 'false'));
  expect(strictGappy.skipped.find((c: { name: string }) => c.name === 'score').reason)
    .toContain('allow_blanks is off');

  const mostly = 'reading\n1\n2\n3\npending';
  expect(JSON.parse(await runWasm(page, mostly, 'auto', 'present', 'columns', 'NA,N/A,NULL,null,None,nan', 'true', '0.75')).columns[0].values)
    .toEqual([1, 2, 3, null]);

  const customNulls = 'x\n1\nmissing\n2';
  expect(JSON.parse(await runWasm(page, customNulls, 'auto', 'present', 'columns', 'missing')).columns[0].missing)
    .toBe(1);
});

test('csv-numeric-column-extractor wasm handles quoted fields, IDs and the cap boundary', async ({ page }) => {
  await page.goto('/tools/csv-numeric-column-extractor/');
  await page.waitForSelector('#in-data');

  await expect(runWasm(page, 'label,value\n"alpha, beta",1\n"line\nwrap",2', 'comma', 'present', 'csv'))
    .resolves.toBe('value\n1\n2');
  expect(JSON.parse(await runWasm(page, 'zip,amount\n01234,5\n98765,6')).skipped[0].name).toBe('zip');
  await expect(runWasm(page, 'a'.repeat(1_000_001))).rejects.toThrow(/maximum is 1000000 bytes/);
});

test('csv-numeric-column-extractor page renders exact CSV and reacts to controls', async ({ page }) => {
  await page.goto('/tools/csv-numeric-column-extractor/');
  await page.fill('#in-data', SAMPLE);
  await page.selectOption('#in-output', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('id,score', { timeout: 15_000 });
  expect(await out.textContent()).toBe(CSV_ONLY);

  await page.selectOption('#in-output', 'names');
  await expect(out).toHaveText('id\nscore', { timeout: 15_000 });
});

test('csv-numeric-column-extractor deep link pre-fills params and computes', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'reading\n1\n2\n3\npending',
    delimiter: 'auto',
    header: 'present',
    output: 'columns',
    null_tokens: 'NA,N/A,NULL,null,None,nan',
    allow_blanks: 'true',
    min_numeric_ratio: '0.75',
    normalize: 'true',
  });
  await page.goto(`/tools/csv-numeric-column-extractor/?${params.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue('reading\n1\n2\n3\npending', { timeout: 15_000 });
  await expect(page.locator('#in-min_numeric_ratio')).toHaveValue('0.75');
  await expect(page.locator('#tool-output')).toContainText('"numeric_ratio": 0.75', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('null');
});

test('csv-numeric-column-extractor page ships a runnable generated CLI example', async ({ page }) => {
  await page.goto('/tools/csv-numeric-column-extractor/');
  await page.waitForSelector('#in-data');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool csv-numeric-column-extractor');
  expect(cli).toContain('id,name,score');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

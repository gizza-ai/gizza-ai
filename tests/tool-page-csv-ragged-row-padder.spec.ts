import { test, expect } from './fixtures';

const sample = 'name,email,age\nAda,ada@example.com,37\nGrace,grace@example.com\nLinus,linus@example.com,54,extra';

async function runWasm(
  page,
  params: Partial<{
    input: string;
    width: number;
    width_from: string;
    long_rows: string;
    pad_value: string;
    header: boolean;
    delimiter: string;
    drop_empty_rows: boolean;
    line_ending: string;
    output: string;
  }> = {}
): Promise<string> {
  const p = {
    input: sample,
    width: 0,
    width_from: 'header',
    long_rows: 'truncate',
    pad_value: '',
    header: true,
    delimiter: 'auto',
    drop_empty_rows: true,
    line_ending: 'lf',
    output: 'csv',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/csv-ragged-row-padder/gizza_ai_csv_ragged_row_padder_web.js');
    await mod.default('/tools/csv-ragged-row-padder/gizza_ai_csv_ragged_row_padder_web_bg.wasm');
    return mod.run(
      args.input,
      args.width,
      args.width_from,
      args.long_rows,
      args.pad_value,
      args.header,
      args.delimiter,
      args.drop_empty_rows,
      args.line_ending,
      args.output
    );
  }, p);
}

test('csv-ragged-row-padder page pads short rows and truncates long rows', async ({ page }) => {
  await page.goto('/tools/csv-ragged-row-padder/');
  await page.waitForSelector('#in-input');
  await page.fill('#in-input', sample);
  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15_000 })
    .toBe('name,email,age\nAda,ada@example.com,37\nGrace,grace@example.com,\nLinus,linus@example.com,54\n');
});

test('csv-ragged-row-padder deep link prefills report mode and flags long rows', async ({ page }) => {
  const input = 'a,b\n1,2,3\n4';
  await page.goto(
    '/tools/csv-ragged-row-padder/?input=' +
      encodeURIComponent(input) +
      '&width=0&width_from=header&long_rows=flag&pad_value=NULL&header=true&delimiter=comma&drop_empty_rows=true&line_ending=lf&output=report'
  );
  await expect(page.locator('#in-input')).toHaveValue(input, { timeout: 15_000 });
  await expect(page.locator('#in-width')).toHaveValue('0');
  await expect(page.locator('#in-width_from')).toHaveValue('header');
  await expect(page.locator('#in-long_rows')).toHaveValue('flag');
  await expect(page.locator('#in-pad_value')).toHaveValue('NULL');
  await expect(page.locator('#in-header')).toBeChecked();
  await expect(page.locator('#in-delimiter')).toHaveValue('comma');
  await expect(page.locator('#in-drop_empty_rows')).toBeChecked();
  await expect(page.locator('#in-line_ending')).toHaveValue('lf');
  await expect(page.locator('#in-output')).toHaveValue('report');
  await expect(page.locator('#tool-output')).toContainText('Long rows flagged: 1', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('line 2: 3 fields -> flagged, left at 3 fields');
});

test('csv-ragged-row-padder covers advertised modes, delimiters, line endings and booleans', async ({ page }) => {
  await page.goto('/tools/csv-ragged-row-padder/');
  await page.waitForSelector('#in-input');

  expect(await runWasm(page, { input: 'a,b,c\n1,2,3,4,5\n', long_rows: 'merge' })).toBe('a,b,c\n1,2,"3,4,5"\n');
  expect(await runWasm(page, { input: 'a,b\n1,2\n3,4,5\n', long_rows: 'drop' })).toBe('a,b\n1,2\n');
  expect(await runWasm(page, { input: '1,2\n3,4\n5,6,7,8\n', header: false, width_from: 'max', pad_value: 'NULL' })).toBe('1,2,NULL,NULL\n3,4,NULL,NULL\n5,6,7,8\n');
  expect(await runWasm(page, { input: 'a;b;c\n1;2\n', delimiter: 'semicolon' })).toBe('a;b;c\n1;2;\n');
  expect(await runWasm(page, { input: 'a\tb\tc\n1\t2\n', delimiter: 'tab', pad_value: 'NA' })).toBe('a\tb\tc\n1\t2\tNA\n');
  expect(await runWasm(page, { input: 'a|b|c\n1|2\n', delimiter: 'pipe', line_ending: 'crlf' })).toBe('a|b|c\r\n1|2|\r\n');
  expect(await runWasm(page, { input: 'a,b,c\n\n1,2\n', drop_empty_rows: false, output: 'report' })).toContain('Short rows padded: 1');
});

test('csv-ragged-row-padder generated CLI example is generic and parseable', async ({ page }) => {
  await page.goto('/tools/csv-ragged-row-padder/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool csv-ragged-row-padder');
  expect(cli).toContain('name,email,age');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

import { test, expect } from './fixtures';

const sample = 'sku,price,weight\nA1,1234.5,0.5\nB2,7,12.25\nC3,n/a,1.005';

type Params = Partial<{
  data: string;
  columns: string;
  decimals: string;
  rounding: string;
  notation: string;
  grouping: string;
  group_separator: string;
  decimal_separator: string;
  sign: string;
  prefix: string;
  suffix: string;
  input_decimal: string;
  non_numeric: string;
  has_header: string;
  delimiter: string;
  quote_style: string;
  output: string;
}>;

async function runWasm(page, params: Params = {}): Promise<string> {
  const p = {
    data: sample,
    columns: 'price',
    decimals: '2',
    rounding: 'half_up',
    notation: 'standard',
    grouping: 'none',
    group_separator: 'comma',
    decimal_separator: 'period',
    sign: 'auto',
    prefix: '',
    suffix: '',
    input_decimal: 'auto',
    non_numeric: 'keep',
    has_header: 'true',
    delimiter: 'auto',
    quote_style: 'minimal',
    output: 'csv',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/csv-column-number-formatter/gizza_ai_csv_column_number_formatter_web.js');
    await mod.default('/tools/csv-column-number-formatter/gizza_ai_csv_column_number_formatter_web_bg.wasm');
    return mod.run(
      args.data,
      args.columns,
      args.decimals,
      args.rounding,
      args.notation,
      args.grouping,
      args.group_separator,
      args.decimal_separator,
      args.sign,
      args.prefix,
      args.suffix,
      args.input_decimal,
      args.non_numeric,
      args.has_header,
      args.delimiter,
      args.quote_style,
      args.output,
    );
  }, p);
}

test('csv-column-number-formatter page applies fixed decimals to one column', async ({ page }) => {
  await page.goto('/tools/csv-column-number-formatter/');
  await page.waitForSelector('#in-data');
  await page.fill('#in-data', sample);
  await page.fill('#in-columns', 'price');

  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15_000 })
    .toBe('sku,price,weight\nA1,1234.50,0.5\nB2,7.00,12.25\nC3,n/a,1.005\n');
});

test('csv-column-number-formatter supports deep-linked accounting currency output', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'account,balance\nrevenue,120450.5\nrefunds,-2310.75',
    columns: 'balance',
    decimals: '2',
    rounding: 'half_up',
    notation: 'standard',
    grouping: 'thousands',
    group_separator: 'comma',
    decimal_separator: 'period',
    sign: 'parens',
    prefix: '$',
    suffix: '',
    input_decimal: 'auto',
    non_numeric: 'keep',
    has_header: 'true',
    delimiter: 'auto',
    quote_style: 'minimal',
    output: 'csv',
  });
  await page.goto(`/tools/csv-column-number-formatter/?${params.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue('account,balance\nrevenue,120450.5\nrefunds,-2310.75', { timeout: 15_000 });
  await expect(page.locator('#in-columns')).toHaveValue('balance');
  await expect(page.locator('#in-grouping')).toHaveValue('thousands');
  await expect(page.locator('#in-sign')).toHaveValue('parens');
  await expect(page.locator('#in-prefix')).toHaveValue('$');
  await expect(page.locator('#tool-output')).toHaveText('account,balance\nrevenue,"$120,450.50"\nrefunds,"($2,310.75)"\n', { timeout: 15_000 });
});

test('csv-column-number-formatter wasm covers advertised values, limits and CLI example', async ({ page }) => {
  await page.goto('/tools/csv-column-number-formatter/');
  await page.waitForSelector('#in-data');

  expect(await runWasm(page, { data: 'v\n1.005', columns: 'v', decimals: '2', rounding: 'half_up' })).toBe('v\n1.01\n');
  expect(await runWasm(page, { data: 'v\n2.5\n1.5', columns: 'v', decimals: '0', rounding: 'half_even' })).toBe('v\n2\n2\n');
  expect(await runWasm(page, { data: 'v\n2.9\n-2.1', columns: 'v', decimals: '0', rounding: 'ceil' })).toBe('v\n3\n-2\n');
  expect(await runWasm(page, { data: 'v\n2.9\n-2.1', columns: 'v', decimals: '0', rounding: 'floor' })).toBe('v\n2\n-3\n');
  expect(await runWasm(page, { data: 'v\n2.9\n-2.9', columns: 'v', decimals: '0', rounding: 'truncate' })).toBe('v\n2\n-2\n');
  expect(await runWasm(page, { data: 'v\n2.5\n2.51', columns: 'v', decimals: '0', rounding: 'half_down' })).toBe('v\n2\n3\n');

  expect(await runWasm(page, { data: 'v\n1234567', columns: 'v', grouping: 'indian' })).toBe('v\n"12,34,567.00"\n');
  expect(await runWasm(page, { data: 'v\n1234567', columns: 'v', notation: 'compact' })).toBe('v\n1.23M\n');
  expect(await runWasm(page, { data: 'v\n1234567', columns: 'v', notation: 'scientific' })).toBe('v\n1.23e+6\n');
  expect(await runWasm(page, { data: 'v\n0.452', columns: 'v', notation: 'percent', decimals: '1' })).toBe('v\n45.2%\n');
  expect(await runWasm(page, { data: 'v\n1234.56', columns: 'v', grouping: 'thousands', group_separator: 'period', decimal_separator: 'comma' })).toBe('v\n"1.234,56"\n');
  expect(await runWasm(page, { data: 'v;note\n1.234,56;eu', columns: 'v', input_decimal: 'comma', delimiter: 'semicolon' })).toBe('v;note\n1234.56;eu\n');
  expect(await runWasm(page, { data: 'v\n12345', columns: 'v', decimals: '-2' })).toBe('v\n12300\n');
  expect(await runWasm(page, { data: 'v\n123.456789012345678', columns: 'v', decimals: '15' })).toBe('v\n123.456789012345678\n');
  expect(await runWasm(page, { data: 'v\n7\nn/a', columns: 'v', non_numeric: 'blank' })).toBe('v\n7.00\n""\n');
  expect(await runWasm(page, { data: '1\n2', columns: '1', has_header: 'false' })).toBe('1.00\n2.00\n');
  expect(await runWasm(page, { data: 'v\n7', columns: 'v', sign: 'always', prefix: '$', suffix: ' USD' })).toBe('v\n+$7.00 USD\n');
  expect(await runWasm(page, { data: 'v\n7', columns: 'v', output: 'report' })).toBe('column,cells_formatted,cells_unchanged,non_numeric\nv,1,0,0\nTOTAL,1,0,0\n');
  await expect(runWasm(page, { data: 'v\nn/a', columns: 'v', non_numeric: 'error' })).rejects.toThrow(/not a number/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool csv-column-number-formatter');
  expect(cli).toContain('sku,price,weight');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

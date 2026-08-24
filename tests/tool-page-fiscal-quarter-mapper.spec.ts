import { test, expect } from './fixtures';

const SAMPLE = 'invoice,closed,amount\nA-1001,2025-10-14,4200\nA-1002,2026-01-07,1875\nA-1003,2026-04-25,3060';

async function runWasm(
  page: any,
  params: Partial<{
    input: string;
    column: string;
    fiscal_start_month: string;
    fiscal_year_naming: string;
    quarter_label: string;
    fiscal_year_label: string;
    add_fiscal_year: string;
    add_quarter_dates: string;
    add_fiscal_month: string;
    add_quarter_position: string;
    date_order: string;
    on_error: string;
    header: string;
    delimiter: string;
    output: string;
  }> = {},
): Promise<string> {
  const p = {
    input: SAMPLE,
    column: 'closed',
    fiscal_start_month: 'october',
    fiscal_year_naming: 'end',
    quarter_label: 'q-fy',
    fiscal_year_label: 'fy-yyyy',
    add_fiscal_year: 'true',
    add_quarter_dates: 'false',
    add_fiscal_month: 'false',
    add_quarter_position: 'false',
    date_order: 'auto',
    on_error: 'blank',
    header: 'true',
    delimiter: 'auto',
    output: 'csv',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/fiscal-quarter-mapper/gizza_ai_fiscal_quarter_mapper_web.js');
    await mod.default('/tools/fiscal-quarter-mapper/gizza_ai_fiscal_quarter_mapper_web_bg.wasm');
    return mod.run(
      args.input,
      args.column,
      args.fiscal_start_month,
      args.fiscal_year_naming,
      args.quarter_label,
      args.fiscal_year_label,
      args.add_fiscal_year,
      args.add_quarter_dates,
      args.add_fiscal_month,
      args.add_quarter_position,
      args.date_order,
      args.on_error,
      args.header,
      args.delimiter,
      args.output,
    );
  }, p);
}

test('fiscal-quarter-mapper page maps a CSV date column to fiscal quarters', async ({ page }) => {
  await page.goto('/tools/fiscal-quarter-mapper/');
  await page.fill('#in-input', SAMPLE);
  await page.fill('#in-column', 'closed');
  await page.selectOption('#in-fiscal_start_month', 'october');

  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15_000 })
    .toBe(
      'invoice,closed,amount,fiscal_quarter,fiscal_year\n' +
        'A-1001,2025-10-14,4200,Q1 FY2026,FY2026\n' +
        'A-1002,2026-01-07,1875,Q2 FY2026,FY2026\n' +
        'A-1003,2026-04-25,3060,Q3 FY2026,FY2026\n',
    );
});

test('fiscal-quarter-mapper deep link prefills report mode and day-first dates', async ({ page }) => {
  const input = 'closed,amount\n03/04/2024,120\n25/12/2024,340\nnot a date,55';
  const qs = new URLSearchParams({
    input,
    column: 'closed',
    fiscal_start_month: 'april',
    fiscal_year_naming: 'end',
    quarter_label: 'q-fy',
    fiscal_year_label: 'fy-yyyy',
    add_fiscal_year: 'true',
    add_quarter_dates: 'false',
    add_fiscal_month: 'false',
    add_quarter_position: 'false',
    date_order: 'auto',
    on_error: 'blank',
    header: 'true',
    delimiter: 'auto',
    output: 'report',
  });

  await page.goto(`/tools/fiscal-quarter-mapper/?${qs.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(input, { timeout: 15_000 });
  await expect(page.locator('#in-fiscal_start_month')).toHaveValue('april');
  await expect(page.locator('#in-output')).toHaveValue('report');
  await expect(page.locator('#tool-output')).toContainText('date order: day-first', { timeout: 15_000 });
  await expect(page.locator('#tool-output')).toContainText('unreadable 1');
});

test('fiscal-quarter-mapper wasm covers advertised values, booleans and CLI example', async ({ page }) => {
  await page.goto('/tools/fiscal-quarter-mapper/');

  expect(await runWasm(page, { fiscal_start_month: 'january' })).toContain('Q4 FY2025,FY2025');
  expect(await runWasm(page, { fiscal_start_month: 'april', fiscal_year_label: 'range' })).toContain('Q3 2025-2026,2025-2026');
  expect(await runWasm(page, { fiscal_start_month: 'july', quarter_label: 'yyyyqn', add_fiscal_year: 'false' })).toContain('2026Q2');
  expect(await runWasm(page, { fiscal_year_naming: 'start' })).toContain('Q1 FY2025,FY2025');
  expect(await runWasm(page, { quarter_label: 'fy-q' })).toContain('FY2026-Q1');
  expect(await runWasm(page, { quarter_label: 'yyyy-qn' })).toContain('2026-Q1');
  expect(await runWasm(page, { quarter_label: 'qn' })).toContain('Q1');
  expect(await runWasm(page, { quarter_label: 'n' })).toContain(',1,FY2026');
  expect(await runWasm(page, { fiscal_year_label: 'yyyy' })).toContain('Q1 2026,2026');
  expect(await runWasm(page, { fiscal_year_label: 'fy-yy' })).toContain('Q1 FY26,FY26');
  expect(await runWasm(page, { fiscal_year_label: 'range-short' })).toContain('Q1 2025-26,2025-26');
  expect(
    await runWasm(page, {
      add_fiscal_year: 'false',
      add_quarter_dates: 'true',
      add_fiscal_month: 'true',
      add_quarter_position: 'true',
    }),
  ).toContain('fiscal_quarter_start,fiscal_quarter_end,fiscal_month,day_of_quarter,days_in_quarter');
  expect(await runWasm(page, { input: 'closed;amount\n2025-10-14;4200', delimiter: 'semicolon' })).toContain('closed;amount;fiscal_quarter;fiscal_year');
  expect(await runWasm(page, { output: 'json' })).toContain('"column": "closed"');
  await expect(runWasm(page, { on_error: 'error', input: 'closed\nnot-a-date' })).rejects.toThrow(/row 1 \(line 2\), column 'closed': cannot read 'not-a-date'/);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool fiscal-quarter-mapper');
  expect(cli).toContain('invoice,closed,amount');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

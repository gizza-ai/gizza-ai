import { test, expect } from './fixtures';

const tool = '/tools/csv-date-normalizer/';
const messy = 'id,joined\n1,2021-06-01\n2,06/15/2021\n3,15 Jan 2024\n4,not a date';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return text ?? '';
}

async function runWasm(
  page,
  data: string,
  columns = 'auto',
  format = 'iso-auto',
  customFormat = '',
  dateOrder = 'auto',
  yearPivot = '68',
  excelSerial = 'true',
  onError = 'keep',
  hasHeader = 'true',
  delimiter = 'auto',
  output = 'csv',
) {
  return await page.evaluate(
    async ({ data, columns, format, customFormat, dateOrder, yearPivot, excelSerial, onError, hasHeader, delimiter, output }) => {
      const mod = await import('/tools/csv-date-normalizer/gizza_ai_csv_date_normalizer_web.js');
      await mod.default('/tools/csv-date-normalizer/gizza_ai_csv_date_normalizer_web_bg.wasm');
      return mod.run(data, columns, format, customFormat, dateOrder, yearPivot, excelSerial, onError, hasHeader, delimiter, output);
    },
    { data, columns, format, customFormat, dateOrder, yearPivot, excelSerial, onError, hasHeader, delimiter, output },
  );
}

test('csv-date-normalizer page normalizes a mixed date column with exact output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-data', messy);
  await page.fill('#in-columns', 'auto');
  await page.selectOption('#in-format', 'iso-auto');
  await page.fill('#in-custom_format', '');
  await page.selectOption('#in-date_order', 'auto');
  await page.fill('#in-year_pivot', '68');
  await page.check('#in-excel_serial');
  await page.selectOption('#in-on_error', 'keep');
  await page.check('#in-has_header');
  await page.fill('#in-delimiter', 'auto');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toContainText('2021-06-15', { timeout: 15000 });
  expect((await outputText(page)).trim()).toBe(
    'id,joined\n1,2021-06-01\n2,2021-06-15\n3,2024-01-15\n4,not a date',
  );
});

test('csv-date-normalizer deep link prefills the report output and a non-default checkbox', async ({ page }) => {
  await page.goto(
    tool +
      '?data=' +
      encodeURIComponent('id,ts\n1,45000\n2,1700000000') +
      '&columns=ts&format=iso-date&custom_format=&date_order=auto&year_pivot=68' +
      '&excel_serial=false&on_error=keep&has_header=true&delimiter=auto&output=report',
  );
  await expect(page.locator('#in-data')).toHaveValue('id,ts\n1,45000\n2,1700000000', { timeout: 15000 });
  await expect(page.locator('#in-excel_serial')).not.toBeChecked();
  await expect(page.locator('#in-has_header')).toBeChecked();
  await expect(page.locator('#in-columns')).toHaveValue('ts');
  await expect(page.locator('#in-output')).toHaveValue('report');

  // excel_serial off: 45000 is no longer a spreadsheet serial, so it is
  // unreadable and listed; the Unix epoch is still recognised by magnitude.
  await expect(page.locator('#tool-output')).toContainText('Converted: 1   Unreadable: 1');
  await expect(page.locator('#tool-output')).toContainText('row 1 (line 2): "45000"');
});

test('csv-date-normalizer wasm covers every advertised format, order, delimiter and policy', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  // Every output format, one real run each.
  const t = 't\n2024-01-15T10:30:00Z';
  const tOffset = 't\n2024-01-15T10:30:00+05:30';
  const d = 'd\n15 Jan 2024';
  expect(await runWasm(page, t, 't', 'iso-auto')).toBe('t\n2024-01-15T10:30:00Z\n');
  expect(await runWasm(page, t, 't', 'iso-date')).toBe('t\n2024-01-15\n');
  expect(await runWasm(page, tOffset, 't', 'iso-datetime')).toBe('t\n2024-01-15T10:30:00\n');
  expect(await runWasm(page, tOffset, 't', 'iso-utc')).toBe('t\n2024-01-15T05:00:00Z\n');
  expect(await runWasm(page, t, 't', 'unix-seconds')).toBe('t\n1705314600\n');
  expect(await runWasm(page, t, 't', 'unix-millis')).toBe('t\n1705314600000\n');
  expect(await runWasm(page, d, 'd', 'us-date')).toBe('d\n01/15/2024\n');
  expect(await runWasm(page, d, 'd', 'eu-date')).toBe('d\n15/01/2024\n');
  expect(await runWasm(page, t, 't', 'sql')).toBe('t\n2024-01-15 10:30:00\n');
  expect(await runWasm(page, d, 'd', 'compact')).toBe('d\n20240115\n');
  expect(await runWasm(page, t, 't', 'rfc2822')).toBe('t\n"Mon, 15 Jan 2024 10:30:00 +0000"\n');
  expect(await runWasm(page, d, 'd', 'custom', '%d %B %Y')).toBe('d\n15 January 2024\n');

  // Day/month order: forced both ways, and inferred from an unambiguous row.
  expect(await runWasm(page, 'd\n03/04/2024', 'd', 'iso-date', '', 'day-first')).toBe('d\n2024-04-03\n');
  expect(await runWasm(page, 'd\n03/04/2024', 'd', 'iso-date', '', 'month-first')).toBe('d\n2024-03-04\n');
  expect(await runWasm(page, 'd\n25/12/2021\n03/04/2021', 'd', 'iso-date', '', 'auto')).toBe('d\n2021-12-25\n2021-04-03\n');

  // Delimiters: sniffed, named, and a bare character. The separator round-trips.
  expect(await runWasm(page, 'id\td\n1\t06/15/2021', 'd', 'iso-date')).toBe('id\td\n1\t2021-06-15\n');
  expect(await runWasm(page, 'id;d\n1;06/15/2021', 'd', 'iso-date', '', 'auto', '68', 'true', 'keep', 'true', 'semicolon')).toBe('id;d\n1;2021-06-15\n');
  expect(await runWasm(page, 'id|d\n1|06/15/2021', 'd', 'iso-date', '', 'auto', '68', 'true', 'keep', 'true', '|')).toBe('id|d\n1|2021-06-15\n');

  // Non-default checkbox states, both ways round.
  expect(await runWasm(page, 'd\n45000', 'd', 'iso-date')).toBe('d\n2023-03-15\n');
  expect(await runWasm(page, 'd\n45000', 'd', 'iso-date', '', 'auto', '68', 'false')).toBe('d\n45000\n');
  expect(await runWasm(page, '06/15/2021,x\n2021-07-04,y', '0', 'iso-date', '', 'auto', '68', 'true', 'keep', 'false')).toBe('2021-06-15,x\n2021-07-04,y\n');

  // Two-digit year pivot.
  expect(await runWasm(page, 'd\n01/02/69', 'd', 'iso-date', '', 'month-first', '68')).toBe('d\n1969-01-02\n');
  expect(await runWasm(page, 'd\n01/02/69', 'd', 'iso-date', '', 'month-first', '99')).toBe('d\n2069-01-02\n');

  // Unreadable-value policy: keep (default), blank, error.
  const ragged = 'd,x\n06/15/2021,a\nnope,b';
  expect(await runWasm(page, ragged, 'd', 'iso-date')).toBe('d,x\n2021-06-15,a\nnope,b\n');
  expect(await runWasm(page, ragged, 'd', 'iso-date', '', 'auto', '68', 'true', 'blank')).toBe('d,x\n2021-06-15,a\n,b\n');
  await expect(runWasm(page, ragged, 'd', 'iso-date', '', 'auto', '68', 'true', 'error')).rejects.toThrow(
    /cannot read 'nope' as a date/,
  );

  // Error paths the page advertises.
  await expect(runWasm(page, 'd\n1', 'd', 'iso-date', '', 'auto', '68', 'true', 'keep', 'true', '::')).rejects.toThrow(/delimiter must be/);
  await expect(runWasm(page, 'd\n2021-06-01', 'd', 'custom', '%Q')).rejects.toThrow(/invalid custom_format/);
  await expect(runWasm(page, 'a,b\n1,2', 'auto')).rejects.toThrow(/no date columns detected/);
});

test('csv-date-normalizer enforces the advertised 5,000,000-byte cap at the boundary', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-data');

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/csv-date-normalizer/gizza_ai_csv_date_normalizer_web.js');
    await mod.default('/tools/csv-date-normalizer/gizza_ai_csv_date_normalizer_web_bg.wasm');
    // 2 + 11 * 454545 + 3 === 5_000_000 bytes exactly.
    const atCap = 'd\n' + '2021-06-01\n'.repeat(454545) + 'xx\n';
    const overCap = 'd\n' + '2021-06-01\n'.repeat(454545) + 'xxx\n';
    const call = (data: string) => {
      try {
        return { ok: true, value: mod.run(data, 'd', 'iso-date', '', 'auto', '68', 'true', 'keep', 'true', 'auto', 'csv').slice(0, 13) };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCapBytes: atCap.length, overCapBytes: overCap.length, atCap: call(atCap), overCap: call(overCap) };
  });

  expect(result.atCapBytes).toBe(5_000_000);
  expect(result.overCapBytes).toBe(5_000_001);
  expect(result.atCap.ok).toBe(true);
  expect(result.atCap.value).toContain('d\n2021-06-01');
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toMatch(/5000001 bytes; the limit is 5000000 bytes/);
});

test('csv-date-normalizer page ships workflow example presets', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
  await page.click('.tool-example-chip:has-text("Custom pattern")');
  await expect(page.locator('#in-format')).toHaveValue('custom');
  await expect(page.locator('#in-custom_format')).toHaveValue('%d %B %Y');
});

import { test, expect } from './fixtures';

const tool = '/tools/date-format-normalizer/';
const mixed =
  'Order 4471 was placed 03/04/2024 and shipped 15/04/2024.\n' +
  'The invoice is dated 22 April 2024 and falls due 2024-05-06.\n' +
  'Support ticket opened Friday, 3 May 2024 at 2:30 PM.';

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return text ?? '';
}

async function runWasm(
  page,
  text: string,
  outputFormat = 'iso',
  customFormat = '',
  separator = 'dash',
  monthStyle = 'full',
  yearStyle = 'four',
  leadingZeros = 'true',
  inputOrder = 'auto',
  twoDigitYearPivot = '68',
  keepTime = 'true',
  timeStyle = '24h',
  outputTimezone = 'source',
  detectTimestamps = 'false',
  outputMode = 'text',
) {
  return await page.evaluate(
    async ({
      text,
      outputFormat,
      customFormat,
      separator,
      monthStyle,
      yearStyle,
      leadingZeros,
      inputOrder,
      twoDigitYearPivot,
      keepTime,
      timeStyle,
      outputTimezone,
      detectTimestamps,
      outputMode,
    }) => {
      const mod = await import('/tools/date-format-normalizer/gizza_ai_date_format_normalizer_web.js');
      await mod.default('/tools/date-format-normalizer/gizza_ai_date_format_normalizer_web_bg.wasm');
      return mod.run(
        text,
        outputFormat,
        customFormat,
        separator,
        monthStyle,
        yearStyle,
        leadingZeros,
        inputOrder,
        twoDigitYearPivot,
        keepTime,
        timeStyle,
        outputTimezone,
        detectTimestamps,
        outputMode,
      );
    },
    {
      text,
      outputFormat,
      customFormat,
      separator,
      monthStyle,
      yearStyle,
      leadingZeros,
      inputOrder,
      twoDigitYearPivot,
      keepTime,
      timeStyle,
      outputTimezone,
      detectTimestamps,
      outputMode,
    },
  );
}

test('date-format-normalizer page rewrites mixed date text with exact output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-text', mixed);
  await page.selectOption('#in-output_format', 'iso');
  await page.fill('#in-custom_format', '');
  await page.selectOption('#in-separator', 'dash');
  await page.selectOption('#in-month_style', 'full');
  await page.selectOption('#in-year_style', 'four');
  await page.check('#in-leading_zeros');
  await page.selectOption('#in-input_order', 'auto');
  await page.fill('#in-two_digit_year_pivot', '68');
  await page.check('#in-keep_time');
  await page.selectOption('#in-time_style', '24h');
  await page.fill('#in-output_timezone', 'source');
  await page.uncheck('#in-detect_timestamps');
  await page.selectOption('#in-output_mode', 'text');

  await expect(page.locator('#tool-output')).toContainText('2024-04-03', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    'Order 4471 was placed 2024-04-03 and shipped 2024-04-15.\n' +
      'The invoice is dated 2024-04-22 and falls due 2024-05-06.\n' +
      'Support ticket opened 2024-05-03T14:30.',
  );
});

test('date-format-normalizer deep link prefills report mode and non-default checkbox states', async ({ page }) => {
  await page.goto(
    tool +
      '?text=' +
      encodeURIComponent('Kickoff 03/04/2024, freeze 15/04/2024, launch 05/06/2024.') +
      '&output_format=dmy&custom_format=&separator=dot&month_style=full&year_style=four' +
      '&leading_zeros=false&input_order=auto&two_digit_year_pivot=68' +
      '&keep_time=false&time_style=24h&output_timezone=source&detect_timestamps=false&output_mode=report',
  );

  await expect(page.locator('#in-text')).toHaveValue('Kickoff 03/04/2024, freeze 15/04/2024, launch 05/06/2024.', {
    timeout: 15000,
  });
  await expect(page.locator('#in-output_format')).toHaveValue('dmy');
  await expect(page.locator('#in-separator')).toHaveValue('dot');
  await expect(page.locator('#in-leading_zeros')).not.toBeChecked();
  await expect(page.locator('#in-keep_time')).not.toBeChecked();
  await expect(page.locator('#in-output_mode')).toHaveValue('report');
  await expect(page.locator('#tool-output')).toContainText('# 3 date string(s) detected');
  await expect(page.locator('#tool-output')).toContainText('line 1, col 9\t03/04/2024\t->\t3.4.2024');
});

test('date-format-normalizer wasm covers advertised formats, ambiguity, timestamps and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-text');

  // Output formats and enum choices.
  expect(await runWasm(page, 'd 5 Jan 2024', 'ymd', '', 'none')).toBe('d 20240105');
  expect(await runWasm(page, 'd 5 Jan 2024', 'dmy', '', 'dot')).toBe('d 05.01.2024');
  expect(await runWasm(page, 'd 5 Jan 2024', 'mdy', '', 'slash')).toBe('d 01/05/2024');
  expect(await runWasm(page, 'd 5 Jan 2024', 'month_day_year', '', 'dash', 'short')).toBe('d Jan 5, 2024');
  expect(await runWasm(page, 'd 5 Jan 2024', 'day_month_year')).toBe('d 5 January 2024');
  expect(await runWasm(page, 'd 2024-01-05T14:30:00Z', 'rfc2822')).toBe('d Fri, 5 Jan 2024 14:30:00 +0000');
  expect(await runWasm(page, 'd 2024-01-05T00:00:00Z', 'unix_seconds')).toBe('d 1704412800');
  expect(await runWasm(page, 'd 2024-01-05T00:00:00Z', 'unix_millis')).toBe('d 1704412800000');
  expect(await runWasm(page, 'd 5 Jan 2024', 'custom', '%d.%m.%Y')).toBe('d 05.01.2024');

  // Ambiguity and non-default checkboxes.
  expect(await runWasm(page, 'd 03/04/2024', 'iso', '', 'dash', 'full', 'four', 'true', 'day_first')).toBe('d 2024-04-03');
  expect(await runWasm(page, 'd 03/04/2024', 'iso', '', 'dash', 'full', 'four', 'true', 'month_first')).toBe('d 2024-03-04');
  expect(await runWasm(page, 'd 15/04/2024 and 03/04/2024')).toBe('d 2024-04-15 and 2024-04-03');
  expect(await runWasm(page, 'd 5 Jan 2024 at 2:30 PM', 'iso', '', 'dash', 'full', 'four', 'true', 'auto', '68', 'false')).toBe('d 2024-01-05');
  expect(await runWasm(page, 'd 5-1-70', 'iso', '', 'dash', 'full', 'four', 'true', 'month_first', '68')).toBe('d 1970-05-01');
  expect(await runWasm(page, 'd 5-1-70', 'iso', '', 'dash', 'full', 'four', 'true', 'month_first', '99')).toBe('d 2070-05-01');

  // Times, timezone conversion, timestamp detection, list/report modes.
  expect(await runWasm(page, 'd 2024-01-05T23:30:00Z', 'iso', '', 'dash', 'full', 'four', 'true', 'auto', '68', 'true', '24h', 'Europe/Berlin')).toBe('d 2024-01-06T00:30:00+01:00');
  expect(await runWasm(page, 'created 1704465000', 'iso', '', 'dash', 'full', 'four', 'true', 'auto', '68', 'true', '24h', 'UTC', 'false')).toBe('created 1704465000');
  expect(await runWasm(page, 'created 1704465000', 'iso', '', 'dash', 'full', 'four', 'true', 'auto', '68', 'true', '24h', 'UTC', 'true')).toBe('created 2024-01-05T14:30:00Z');
  expect(await runWasm(page, 'A 5 Jan 2024 B 6 Jan 2024', 'iso', '', 'dash', 'full', 'four', 'true', 'auto', '68', 'true', '24h', 'source', 'false', 'list')).toBe('2024-01-05\n2024-01-06');
  expect(await runWasm(page, 'A 03/04/2024 B 15/04/2024', 'iso', '', 'dash', 'full', 'four', 'true', 'auto', '68', 'true', '24h', 'source', 'false', 'report')).toContain('numeric day/month order: day-first');

  // Advertised errors.
  await expect(runWasm(page, '', 'iso')).rejects.toThrow(/text is empty/);
  await expect(runWasm(page, 'd 5 Jan 2024', 'custom', '')).rejects.toThrow(/custom_format is empty/);
  await expect(runWasm(page, 'd 5 Jan 2024', 'iso', '', 'bad')).rejects.toThrow(/separator must be/);
});

test('date-format-normalizer enforces the advertised 1,000,000-byte cap at the boundary', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-text');

  const result = await page.evaluate(async () => {
    const mod = await import('/tools/date-format-normalizer/gizza_ai_date_format_normalizer_web.js');
    await mod.default('/tools/date-format-normalizer/gizza_ai_date_format_normalizer_web_bg.wasm');
    const atCap = '2024-01-05' + 'x'.repeat(1_000_000 - 10);
    const overCap = atCap + 'x';
    const call = (text: string) => {
      try {
        return { ok: true, value: mod.run(text, 'iso', '', 'dash', 'full', 'four', 'true', 'auto', '68', 'true', '24h', 'source', 'false', 'text').slice(0, 10) };
      } catch (e) {
        return { ok: false, value: String(e) };
      }
    };
    return { atCapBytes: atCap.length, overCapBytes: overCap.length, atCap: call(atCap), overCap: call(overCap) };
  });

  expect(result.atCapBytes).toBe(1_000_000);
  expect(result.overCapBytes).toBe(1_000_001);
  expect(result.atCap.ok).toBe(true);
  expect(result.atCap.value).toBe('2024-01-05');
  expect(result.overCap.ok).toBe(false);
  expect(result.overCap.value).toMatch(/over the 1000000 byte limit/);
});

test('date-format-normalizer page ships workflow example presets', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(6);

  await page.click('.tool-example-chip:has-text("Sortable 20240105 filename stamps")');
  await expect(page.locator('#in-output_format')).toHaveValue('ymd');
  await expect(page.locator('#in-separator')).toHaveValue('none');
  await expect(page.locator('#tool-output')).toContainText('scan 20240105.pdf', { timeout: 15000 });

  await page.click('.tool-example-chip:has-text("Audit what was detected and why")');
  await expect(page.locator('#in-output_mode')).toHaveValue('report');
  await expect(page.locator('#tool-output')).toContainText('# numeric day/month order: day-first');
});

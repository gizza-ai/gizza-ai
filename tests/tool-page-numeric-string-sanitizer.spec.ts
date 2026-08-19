import { test, expect } from './fixtures';

async function runWasm(page, input: string, decimalSeparator = 'auto', percent = 'strip', magnitudeSuffixes = true, parenthesesNegative = true, decimals = 'auto', onError = 'blank', output = 'values', stats = false) {
  return await page.evaluate(async ({ input, decimalSeparator, percent, magnitudeSuffixes, parenthesesNegative, decimals, onError, output, stats }) => {
    const mod = await import('/tools/numeric-string-sanitizer/gizza_ai_numeric_string_sanitizer_web.js');
    await mod.default('/tools/numeric-string-sanitizer/gizza_ai_numeric_string_sanitizer_web_bg.wasm');
    return mod.run(input, decimalSeparator, percent, magnitudeSuffixes, parenthesesNegative, decimals, onError, output, stats);
  }, { input, decimalSeparator, percent, magnitudeSuffixes, parenthesesNegative, decimals, onError, output, stats });
}

test('numeric-string-sanitizer wasm cleans the default messy column exactly', async ({ page }) => {
  await page.goto('/tools/numeric-string-sanitizer/');
  await page.waitForSelector('#in-input');

  const out = await runWasm(page, '$1,234.50 USD\n(250.00)\n1.2K\n45.2%');
  expect(out).toBe('1234.5\n-250\n1200\n45.2');
});

test('numeric-string-sanitizer wasm covers advertised enum choices and error policies', async ({ page }) => {
  await page.goto('/tools/numeric-string-sanitizer/');
  await page.waitForSelector('#in-input');

  await expect(runWasm(page, '45.2%\n12%', 'dot', 'divide', true, true, '4', 'blank', 'values', true))
    .resolves.toContain('0.4520\n0.1200\n\n--- Summary ---');

  const table = await runWasm(page, '1,200\nn/a\n34 kg', 'auto', 'strip', true, true, 'auto', 'marker', 'table', false);
  expect(table).toBe('original\tvalue\tstatus\n1,200\t1200\tok\nn/a\t#ERROR\terror: no digits found\n34 kg\t34\tok');

  const json = await runWasm(page, '1.234,56', 'comma', 'strip', true, true, '2', 'blank', 'json', false);
  expect(json).toContain('"value": 1234.56, "status": "ok"');

  await expect(runWasm(page, '1', 'bad')).rejects.toThrow(/unknown decimal_separator/);
});

test('numeric-string-sanitizer page renders exact values and honors non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/numeric-string-sanitizer/');
  await page.fill('#in-input', '1.2K\n(250.00)');
  await page.uncheck('#in-magnitude_suffixes');
  await page.uncheck('#in-parentheses_negative');
  await expect(page.locator('#tool-output')).toHaveText('1.2\n250', { timeout: 15_000 });
});

test('numeric-string-sanitizer deep-link prefills fields and outputs percent fractions with stats', async ({ page }) => {
  const qs =
    '?input=' + encodeURIComponent('45.2%\n12%') +
    '&decimal_separator=dot' +
    '&percent=divide' +
    '&magnitude_suffixes=true' +
    '&parentheses_negative=true' +
    '&decimals=4' +
    '&on_error=blank' +
    '&output=values' +
    '&stats=false';
  await page.goto('/tools/numeric-string-sanitizer/' + qs);

  await expect(page.locator('#in-input')).toHaveValue('45.2%\n12%', { timeout: 15_000 });
  await expect(page.locator('#in-decimal_separator')).toHaveValue('dot');
  await expect(page.locator('#in-percent')).toHaveValue('divide');
  await expect(page.locator('#in-decimals')).toHaveValue('4');
  await expect(page.locator('#in-stats')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('0.4520\n0.1200', { timeout: 15_000 });
});

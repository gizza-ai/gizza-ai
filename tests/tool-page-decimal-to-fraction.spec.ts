import { test, expect } from './fixtures';

const tool = '/tools/decimal-to-fraction/';

async function runWasm(page: import('@playwright/test').Page, params: Partial<Record<string, string>> = {}) {
  const p = {
    decimal: '0.625',
    repeating_digits: '',
    tolerance: '',
    max_denominator: '',
    denominator: '',
    rounding: 'nearest',
    reduce: 'true',
    ...params,
  };
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/decimal-to-fraction/gizza_ai_decimal_to_fraction_web.js');
    await mod.default('/tools/decimal-to-fraction/gizza_ai_decimal_to_fraction_web_bg.wasm');
    return mod.run(
      args.decimal,
      args.repeating_digits,
      args.tolerance,
      args.max_denominator,
      args.denominator,
      args.rounding,
      args.reduce,
    );
  }, p);
}

test('decimal-to-fraction page renders exact fraction output', async ({ page }) => {
  await page.goto(tool);
  await page.fill('#in-decimal', '0.625');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('"fraction": "5/8"', { timeout: 20_000 });
  await expect(output).toContainText('"is_exact": true');
  await expect(output).toContainText('0.625 = 5/8 exactly.');
});

test('decimal-to-fraction deep link can prefill approximation controls', async ({ page }) => {
  const qs = new URLSearchParams({
    decimal: '3.14159265',
    repeating_digits: '0',
    tolerance: '0',
    max_denominator: '100',
    denominator: '0',
    rounding: 'nearest',
    reduce: 'true',
  });
  await page.goto(`${tool}?${qs.toString()}`);
  await expect(page.locator('#in-decimal')).toHaveValue('3.14159265', { timeout: 15_000 });
  await expect(page.locator('#in-max_denominator')).toHaveValue('100');
  await expect(page.locator('#in-rounding')).toHaveValue('nearest');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('"fraction": "311/99"', { timeout: 20_000 });
});

test('decimal-to-fraction wasm covers repeats, rounding, reduce, caps, and errors', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-decimal');

  const exact = JSON.parse(await runWasm(page));
  expect(exact.fraction).toBe('5/8');
  expect(exact.mixed_number).toBe('5/8');

  const repeating = JSON.parse(await runWasm(page, { decimal: '0.(3)' }));
  expect(repeating.fraction).toBe('1/3');
  expect(repeating.repeating).toBe(true);

  const fixed = JSON.parse(await runWasm(page, { decimal: '0.31', denominator: '16', rounding: 'down' }));
  expect(fixed.fraction).toBe('1/4');
  expect(fixed.method).toBe('fixed-denominator');

  const unreduced = JSON.parse(await runWasm(page, { decimal: '0.5', denominator: '16', reduce: 'false' }));
  expect(unreduced.fraction).toBe('8/16');
  expect(unreduced.reduced).toBe(false);

  const tolerated = JSON.parse(await runWasm(page, { decimal: '0.142857', tolerance: '0.000001' }));
  expect(tolerated.fraction).toBe('1/7');
  expect(tolerated.method).toBe('tolerance');

  const capped = JSON.parse(await runWasm(page, { decimal: '3.14159265', max_denominator: '100' }));
  expect(capped.denominator).toBeLessThanOrEqual(100);

  await expect(runWasm(page, { decimal: '', denominator: '0' })).rejects.toThrow(/enter a decimal number/);
  await expect(runWasm(page, { decimal: '0.5', denominator: '1000000000001' })).rejects.toThrow(
    /denominator must be between/,
  );
});

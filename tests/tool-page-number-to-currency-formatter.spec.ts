import { test, expect } from './fixtures';

async function runFormatter(page, params) {
  return await page.evaluate(async (p) => {
    const mod = await import('/tools/number-to-currency-formatter/gizza_ai_number_to_currency_formatter_web.js');
    await mod.default('/tools/number-to-currency-formatter/gizza_ai_number_to_currency_formatter_web_bg.wasm');
    return mod.run(
      p.value,
      p.currency,
      p.symbol_style,
      p.position,
      p.symbol_space,
      p.decimals,
      p.rounding,
      p.grouping,
      p.digit_grouping,
      p.group_separator,
      p.decimal_separator,
      p.sign_style,
      p.accounting,
      p.trim_zeros,
    );
  }, params);
}

const defaults = {
  value: '1234.5',
  currency: 'USD',
  symbol_style: 'symbol',
  position: 'before',
  symbol_space: 'false',
  decimals: '2',
  rounding: 'half_up',
  grouping: 'true',
  digit_grouping: 'western',
  group_separator: 'comma',
  decimal_separator: 'period',
  sign_style: 'auto',
  accounting: 'false',
  trim_zeros: 'false',
};

test('number-to-currency-formatter wasm export returns exact default output', async ({ page }) => {
  await page.goto('/tools/number-to-currency-formatter/');
  await page.waitForSelector('#in-value');
  await expect(runFormatter(page, defaults)).resolves.toBe('$1,234.50');
});

test('number-to-currency-formatter wasm export covers advertised options', async ({ page }) => {
  await page.goto('/tools/number-to-currency-formatter/');
  await page.waitForSelector('#in-value');

  await expect(runFormatter(page, {
    ...defaults,
    value: '12345678.9',
    currency: 'INR',
    decimals: '0',
    digit_grouping: 'indian',
  })).resolves.toBe('₹1,23,45,679');

  await expect(runFormatter(page, {
    ...defaults,
    value: '-1234.5',
    accounting: 'true',
  })).resolves.toBe('($1,234.50)');

  await expect(runFormatter(page, {
    ...defaults,
    value: '1234.5',
    currency: 'EUR',
    symbol_style: 'code',
    position: 'after',
    symbol_space: 'true',
    group_separator: 'period',
    decimal_separator: 'comma',
  })).resolves.toBe('1.234,50 EUR');

  await expect(runFormatter(page, {
    ...defaults,
    value: '1.005',
    currency: 'USD',
    symbol_style: 'none',
    grouping: 'false',
    rounding: 'half_even',
  })).resolves.toBe('1.00');

  await expect(runFormatter(page, {
    ...defaults,
    value: '0.123456789',
    currency: 'BTC',
    decimals: '8',
    rounding: 'truncate',
    trim_zeros: 'true',
  })).resolves.toBe('₿0.12345678');
});

test('number-to-currency-formatter page formats default input live', async ({ page }) => {
  await page.goto('/tools/number-to-currency-formatter/');
  await page.fill('#in-value', '999999.999');
  await expect(page.locator('#tool-output')).toContainText('$1,000,000.00', { timeout: 15_000 });
});

test('number-to-currency-formatter deep-link prefills controls and formats accounting output', async ({ page }) => {
  const params = new URLSearchParams({
    value: '-1234.5',
    currency: 'USD',
    symbol_style: 'symbol',
    position: 'before',
    symbol_space: 'false',
    decimals: '2',
    rounding: 'half_up',
    grouping: 'true',
    digit_grouping: 'western',
    group_separator: 'comma',
    decimal_separator: 'period',
    sign_style: 'auto',
    accounting: 'true',
    trim_zeros: 'false',
  });
  await page.goto(`/tools/number-to-currency-formatter/?${params.toString()}`);
  await expect(page.locator('#in-value')).toHaveValue('-1234.5');
  await expect(page.locator('#in-currency')).toHaveValue('USD');
  await expect(page.locator('#in-accounting')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('($1,234.50)', { timeout: 15_000 });
});

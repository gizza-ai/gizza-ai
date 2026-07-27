import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('currency-converter page — direct conversion with exact output', async ({ page }) => {
  await page.goto('/tools/currency-converter/');
  await page.fill('#in-amount', '100');
  await page.fill('#in-from', 'USD');
  await page.fill('#in-to', 'EUR');
  await page.fill('#in-rates', 'USD EUR 0.92');
  await page.fill('#in-precision', '2');
  await expect(page.locator('#tool-output')).toContainText('92.00 EUR', { timeout: 15000 });
  expect(await outputText(page)).toBe('100 USD = 92.00 EUR\nRate: 1 USD = 0.92 EUR');
});

test('currency-converter page — multi-leg path and reverse-rate checkbox off', async ({ page }) => {
  await page.goto('/tools/currency-converter/');
  await page.fill('#in-amount', '100');
  await page.fill('#in-from', 'USD');
  await page.fill('#in-to', 'JPY');
  await page.fill('#in-rates', 'USD EUR 0.9\nEUR JPY 160');
  await page.uncheck('#in-bidirectional');
  await page.fill('#in-precision', '2');
  await expect(page.locator('#tool-output')).toContainText('USD → EUR → JPY', { timeout: 15000 });
  expect(await outputText(page)).toBe('100 USD = 14,400.00 JPY\nRate: 1 USD = 144 JPY\nPath: USD → EUR → JPY (2 legs)');
});

test('currency-converter page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/currency-converter/?amount=100&from=EUR&to=USD&rates=' +
      encodeURIComponent('USD EUR 0.92') +
      '&bidirectional=true&precision=2',
  );
  await expect(page.locator('#in-from')).toHaveValue('EUR', { timeout: 15000 });
  await expect(page.locator('#in-to')).toHaveValue('USD');
  await expect(page.locator('#in-bidirectional')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('108.70 USD', { timeout: 15000 });
  expect(await outputText(page)).toBe('100 EUR = 108.70 USD\nRate: 1 EUR = 1.086957 USD');
});

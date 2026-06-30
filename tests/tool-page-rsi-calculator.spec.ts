import { test, expect } from './fixtures';

const WILDER = '44.34,44.09,44.15,43.61,44.33,44.83,45.10,45.42,45.84,46.08,45.89,46.03,45.61,46.28,46.28,46.00,46.03,46.41,46.22,45.64';

test('rsi-calculator page computes Wilder RSI JSON', async ({ page }) => {
  await page.goto('/tools/rsi-calculator/');
  await page.fill('#in-prices', WILDER);
  await page.fill('#in-period', '14');
  await page.fill('#in-overbought', '70');
  await page.fill('#in-oversold', '30');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('latest_rsi', { timeout: 15000 });
  const json = JSON.parse((await out.textContent())!);
  expect(json.count).toBe(20);
  expect(json.period).toBe(14);
  expect(json.rsi[13]).toBeNull();
  expect(json.rsi[14]).toBeGreaterThan(70);
  expect(json.rsi[14]).toBeLessThan(71);
  expect(json.latest_rsi).toBeGreaterThan(57);
  expect(json.latest_rsi).toBeLessThan(58.5);
  expect(json.latest_signal).toBe('neutral');
});

test('rsi-calculator page supports custom thresholds and deep link prefill', async ({ page }) => {
  await page.goto(
    '/tools/rsi-calculator/?prices=' +
      encodeURIComponent('1 2 1 2 1 2') +
      '&period=2&overbought=60&oversold=40',
  );

  await expect(page.locator('#in-prices')).toHaveValue('1 2 1 2 1 2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('overbought', { timeout: 15000 });
  const json = JSON.parse((await out.textContent())!);
  expect(json.latest_rsi).toBe(68.75);
  expect(json.latest_signal).toBe('overbought');
});

test('rsi-calculator page reports insufficient data clearly', async ({ page }) => {
  await page.goto('/tools/rsi-calculator/');
  await page.fill('#in-prices', '1 2 3');
  await page.fill('#in-period', '14');
  await expect(page.locator('#tool-output')).toContainText('needs at least', { timeout: 15000 });
});

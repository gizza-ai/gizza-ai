import { test, expect } from './fixtures';

// /tools/macd-calculator/ computes the MACD line, signal line and histogram
// in-browser (pure wasm) and renders the result as pretty-printed JSON.
test('macd-calculator computes MACD with custom periods', async ({ page }) => {
  await page.goto('/tools/macd-calculator/');
  await page.fill('#in-prices', '1, 2, 3, 4, 5, 6');
  await page.fill('#in-fast', '2');
  await page.fill('#in-slow', '4');
  await page.fill('#in-signal', '2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"count": 6', { timeout: 15000 });
  await expect(out).toContainText('"fast_period": 2');
  await expect(out).toContainText('"slow_period": 4');
  await expect(out).toContainText('"signal_period": 2');
  await expect(out).toContainText('"macd"');
  await expect(out).toContainText('"signal"');
  await expect(out).toContainText('"histogram"');
  await expect(out).toContainText('"latest_histogram": 0');
});

// Default periods (12/26/9) over a 40-point ramp so the signal line warms up.
test('macd-calculator uses default 12/26/9 periods', async ({ page }) => {
  const prices = Array.from({ length: 40 }, (_, i) => i + 1).join(', ');
  await page.goto('/tools/macd-calculator/');
  await page.fill('#in-prices', prices);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"count": 40', { timeout: 15000 });
  await expect(out).toContainText('"fast_period": 12');
  await expect(out).toContainText('"slow_period": 26');
  await expect(out).toContainText('"signal_period": 9');
  await expect(out).toContainText('"latest_macd": 7');
});

// Query-param deep-link pre-fills the fields and computes.
test('macd-calculator query-param deep-link', async ({ page }) => {
  await page.goto('/tools/macd-calculator/?prices=1,2,3,4,5,6&fast=2&slow=4&signal=2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"count": 6', { timeout: 15000 });
  await expect(out).toContainText('"fast_period": 2');
});

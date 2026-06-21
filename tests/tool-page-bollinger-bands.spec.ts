import { test, expect } from './fixtures';

// /tools/bollinger-bands/ computes Bollinger Bands in-browser (pure wasm).
test('bollinger-bands reports middle/upper/lower bands and %B', async ({ page }) => {
  await page.goto('/tools/bollinger-bands/');
  await page.fill('#in-prices', '1 2 3 4 5');
  await page.fill('#in-period', '5');
  await page.fill('#in-num_std', '2');
  const out = page.locator('#tool-output');
  // period 5 over 1..5 → mean 3 → middle band 3.
  await expect(out).toContainText('Bollinger Bands', { timeout: 15000 });
  await expect(out).toContainText('middle 3');
  await expect(out).toContainText('%B');
});

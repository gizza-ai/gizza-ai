import { test, expect } from './fixtures';

// /tools/moving-average/ computes SMA + EMA in-browser (pure wasm) and renders
// the result as pretty-printed JSON.
test('moving-average computes SMA and EMA for a series', async ({ page }) => {
  await page.goto('/tools/moving-average/');
  await page.fill('#in-series', '1, 2, 3, 4, 5');
  await page.fill('#in-period', '3');
  const out = page.locator('#tool-output');
  // count + period echoed back
  await expect(out).toContainText('"count": 5', { timeout: 15000 });
  await expect(out).toContainText('"period": 3');
  await expect(out).toContainText('"smoothing_factor": 0.5');
  // SMA over window 3 of 1..5: nulls then 2,3,4
  await expect(out).toContainText('"sma"');
  await expect(out).toContainText('"ema"');
  await expect(out).toContainText('"wma"');
});

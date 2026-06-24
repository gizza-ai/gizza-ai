import { test, expect } from './fixtures';

// /tools/outlier-detector/ flags outliers in-browser (pure wasm).
test('outlier-detector flags an outlier with z-score and IQR', async ({ page }) => {
  await page.goto('/tools/outlier-detector/');
  // The two number fields are pre-filled from the schema defaults (3 and 1.5).
  await expect(page.locator('#in-z_threshold')).toHaveValue('3');
  await expect(page.locator('#in-iqr_k')).toHaveValue('1.5');

  await page.fill('#in-numbers', '10, 11, 12, 10, 9, 11, 100');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('count: 7', { timeout: 15000 });
  // IQR method flags 100 at the default k=1.5.
  await expect(out).toContainText('IQR method');
  await expect(out).toContainText('100 (index 6)');
  await expect(out).toContainText('z-score method');
  await expect(out).toContainText('modified z-score method');
});

test('outlier-detector honours a custom z-score threshold', async ({ page }) => {
  await page.goto('/tools/outlier-detector/');
  await page.fill('#in-numbers', '10, 11, 12, 10, 9, 11, 100');
  await page.fill('#in-z_threshold', '2');
  const out = page.locator('#tool-output');
  // At threshold 2 the z-score method now flags 100 too.
  await expect(out).toContainText('z-score method (threshold 2)', { timeout: 15000 });
  await expect(out).toContainText('100 (index 6)');
});

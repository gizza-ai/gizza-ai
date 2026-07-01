import { test, expect } from './fixtures';

// /tools/z-score-normalize/ standardizes a list of numbers in-browser (pure wasm)
// and renders the transformed values as text (header + one value per line).
test('z-score-normalize standardizes a number list (z-score)', async ({ page }) => {
  await page.goto('/tools/z-score-normalize/');
  await page.fill('#in-numbers', '2, 4, 4, 4, 5, 5, 7, 9');
  await page.selectOption('#in-method', 'z-score');
  const out = page.locator('#tool-output');
  // population std dev: mean 5, std dev 2, so first value = (2-5)/2 = -1.5
  await expect(out).toContainText('mean 5', { timeout: 15000 });
  await expect(out).toContainText('std dev 2');
  await expect(out).toContainText('-1.5');
});

test('z-score-normalize rescales with min-max', async ({ page }) => {
  await page.goto('/tools/z-score-normalize/');
  await page.fill('#in-numbers', '2, 4, 4, 4, 5, 5, 7, 9');
  await page.selectOption('#in-method', 'min-max');
  const out = page.locator('#tool-output');
  // min 2 -> 0, max 9 -> 1
  await expect(out).toContainText('min-max', { timeout: 15000 });
  await expect(out).toContainText('min 2');
  await expect(out).toContainText('max 9');
});

test('z-score-normalize exposes max-abs and robust scaling methods', async ({ page }) => {
  await page.goto('/tools/z-score-normalize/');
  await page.fill('#in-numbers', '-8, -4, 0, 2, 8');
  await page.selectOption('#in-method', 'max-abs');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('max-abs', { timeout: 15000 });
  await expect(out).toContainText('max abs 8');

  await page.fill('#in-numbers', '1, 2, 3, 4, 5');
  await page.selectOption('#in-method', 'robust');
  await expect(out).toContainText('robust', { timeout: 15000 });
  await expect(out).toContainText('median 3');
  await expect(out).toContainText('IQR 2');
});

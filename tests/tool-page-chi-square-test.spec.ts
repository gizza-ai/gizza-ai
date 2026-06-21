import { test, expect } from './fixtures';

// /tools/chi-square-test/ runs Pearson's chi-square test in-browser (pure wasm).
test('chi-square goodness-of-fit reports statistic, df and p-value', async ({ page }) => {
  await page.goto('/tools/chi-square-test/');
  await page.selectOption('#in-mode', 'goodness-of-fit');
  await page.fill('#in-observed', '16 15 19 17 11 13');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('chi-square: 2.692308', { timeout: 15000 });
  await expect(out).toContainText('degrees of freedom: 5');
  await expect(out).toContainText('p-value: 0.747295');
  await expect(out).toContainText('fail to reject');
});

test('chi-square goodness-of-fit with expected ratios', async ({ page }) => {
  await page.goto('/tools/chi-square-test/');
  await page.selectOption('#in-mode', 'goodness-of-fit');
  await page.fill('#in-observed', '315 108 101 32');
  await page.fill('#in-expected', '9 3 3 1');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('chi-square: 0.470024', { timeout: 15000 });
  await expect(out).toContainText('p-value: 0.925426');
});

test('chi-square contingency table reports Cramér\'s V', async ({ page }) => {
  await page.goto('/tools/chi-square-test/');
  await page.selectOption('#in-mode', 'contingency');
  await page.fill('#in-observed', '10 20\n30 40');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('test: contingency', { timeout: 15000 });
  await expect(out).toContainText('chi-square: 0.793651');
  await expect(out).toContainText('degrees of freedom: 1');
  await expect(out).toContainText("Cramér's V");
});

test('chi-square 2×2 with Yates continuity correction', async ({ page }) => {
  await page.goto('/tools/chi-square-test/');
  await page.selectOption('#in-mode', 'contingency');
  await page.fill('#in-observed', '10 20\n30 40');
  await page.check('#in-yates');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('chi-square: 0.446429', { timeout: 15000 });
  await expect(out).toContainText("Yates' continuity correction: applied");
});

import { test, expect } from './fixtures';

const output = (page) =>
  page.locator('#tool-output').evaluate((el) => el.textContent?.trim() ?? '');

test('drawdown-analyzer page reports max drawdown and recovery details', async ({ page }) => {
  await page.goto('/tools/drawdown-analyzer/');
  await page.fill('#in-series', '100\n120\n90\n110\n130');
  await page.selectOption('#in-series_type', 'equity');
  await page.selectOption('#in-frequency', 'period');
  await page.fill('#in-top_n', '2');
  await page.fill('#in-recovery_cagr', '10');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('max drawdown: -25.0000%', { timeout: 15_000 });
  expect(await output(page)).toContain('peak 120 at #2 → trough 90 at #3');
  expect(await output(page)).toContain('recovered after 2 periods at #5');
  expect(await output(page)).toContain('≈ 3.02 years at 10.00% a year');
  expect(await output(page)).toContain('underwater plot');
});

test('drawdown-analyzer deep-link applies returns, dates and header checkbox', async ({ page }) => {
  const series = encodeURIComponent('month,return\n2024-01-31,2%\n2024-02-29,-5%\n2024-03-31,3%\n2024-04-30,-8%\n2024-05-31,7.5%');
  await page.goto(
    `/tools/drawdown-analyzer/?series=${series}&series_type=returns&frequency=monthly&has_header=true&top_n=1&recovery_cagr=8`,
  );

  await expect(page.locator('#in-series_type')).toHaveValue('returns', { timeout: 15_000 });
  await expect(page.locator('#in-frequency')).toHaveValue('monthly');
  await expect(page.locator('#in-has_header')).toBeChecked();
  await expect(page.locator('#in-top_n')).toHaveValue('1');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('series: 5 returns observations (monthly), 2024-01-31 → 2024-05-31', { timeout: 15_000 });
  expect(await output(page)).toContain('top 1 of 1 drawdowns (deepest first):');
  expect(await output(page)).toContain('2024-01-31');
  expect(await output(page)).toContain('Educational only');
});

test('drawdown-analyzer page validates cap boundary and non-default checkbox', async ({ page }) => {
  await page.goto('/tools/drawdown-analyzer/');
  await page.fill('#in-series', 'value\n100\n80\n100');
  await page.check('#in-has_header');
  await page.fill('#in-top_n', '20');

  await expect(page.locator('#in-has_header')).toBeChecked();
  await expect(page.locator('#in-top_n')).toHaveValue('20');
  await expect(page.locator('#tool-output')).toContainText('max drawdown: -20.0000%', { timeout: 15_000 });
});

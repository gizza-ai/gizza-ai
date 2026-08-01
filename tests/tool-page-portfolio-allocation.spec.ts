import { test, expect } from './fixtures';

const SAMPLE = 'Name, Value, Asset, Sector, Account\nAAPL, 6000, Stocks, Technology, Brokerage\nBND, 3000, Bonds, Bonds, IRA\nCash, 1000, Cash, Cash, Savings';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('portfolio-allocation page calculates asset allocation', async ({ page }) => {
  await page.goto('/tools/portfolio-allocation/');
  await page.fill('#in-input', SAMPLE);
  await page.fill('#in-currency', '$');
  await expect(page.locator('#tool-output')).toContainText('Allocation by asset class — $10,000.00 total across 3 holdings', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('Stocks  $6,000.00   60.00%');
  expect(out).toContain('Bonds   $3,000.00   30.00%');
  expect(out).toContain('Cash    $1,000.00   10.00%');
  expect(out).toContain('Concentration (HHI): 4600 — Highly concentrated');
});

test('portfolio-allocation supports sector grouping, top-n folding, and non-default currency', async ({ page }) => {
  await page.goto('/tools/portfolio-allocation/');
  await page.fill('#in-input', SAMPLE);
  await page.selectOption('#in-group_by', 'sector');
  await page.fill('#in-top_n', '2');
  await page.fill('#in-currency', 'USD ');
  await expect(page.locator('#tool-output')).toContainText('Allocation by sector — USD10,000.00 total across 3 holdings', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('Technology  USD6,000.00   60.00%');
  expect(out).toContain('Bonds       USD3,000.00   30.00%');
  expect(out).toContain('Other       USD1,000.00   10.00%');
});

test('portfolio-allocation supports holding grouping and label sorting', async ({ page }) => {
  await page.goto('/tools/portfolio-allocation/');
  await page.fill('#in-input', 'Zeta, 50\nAlpha, 30\nBeta, 20');
  await page.fill('#in-currency', '$');
  await page.selectOption('#in-group_by', 'holding');
  await page.selectOption('#in-sort', 'label');
  await expect(page.locator('#tool-output')).toContainText('Allocation by holding — $100.00 total across 3 holdings', { timeout: 15000 });
  const out = await outputText(page);
  expect(out.indexOf('Alpha')).toBeLessThan(out.indexOf('Beta'));
  expect(out.indexOf('Beta')).toBeLessThan(out.indexOf('Zeta'));
});

test('portfolio-allocation deep-link pre-fills and auto-runs', async ({ page }) => {
  const params = new URLSearchParams({
    input: SAMPLE,
    group_by: 'account',
    sort: 'value',
    top_n: '0',
    currency: '€',
  });
  await page.goto(`/tools/portfolio-allocation/?${params.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-group_by')).toHaveValue('account');
  await expect(page.locator('#in-currency')).toHaveValue('€');
  await expect(page.locator('#tool-output')).toContainText('Allocation by account — €10,000.00 total across 3 holdings', { timeout: 15000 });
});

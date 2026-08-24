import { test, expect } from './fixtures';

const POSITIONS = 'AAPL, 50, 150, 187.50, 9.99\nTSLA, 10, 250, 220';

async function outputText(page) {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('portfolio-pnl page calculates exact portfolio P/L', async ({ page }) => {
  await page.goto('/tools/portfolio-pnl/');
  await page.fill('#in-positions', POSITIONS);
  await page.fill('#in-tax_rate', '15');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Portfolio P/L: +$1,565.01 on $10,000.00 cost basis (+15.65%)', { timeout: 15_000 });
  const text = await outputText(page);
  expect(text).toContain('AAPL');
  expect(text).toContain('+$1,865.01');
  expect(text).toContain('TSLA');
  expect(text).toContain('-$300.00');
  expect(text).toContain('Tax @ 15%');
  expect(text).toContain('After-tax P/L');
});

test('portfolio-pnl page supports deep-linked short rows, fees, and sorting', async ({ page }) => {
  const params = new URLSearchParams({
    positions: 'AAPL, 50, 150, 187.50\nGME, 100, 40, 25, 5, 0, 60, short\nTSLA, 10, 250, 220',
    side: 'long',
    fee_percent: '0.1',
    tax_rate: '0',
    sort: 'pnl',
    currency: '$',
  });

  await page.goto(`/tools/portfolio-pnl/?${params.toString()}`);
  await expect(page.locator('#in-side')).toHaveValue('long');
  await expect(page.locator('#in-sort')).toHaveValue('pnl');
  await expect(page.locator('#in-fee_percent')).toHaveValue('0.1');
  await expect(page.locator('#tool-output')).toContainText('Portfolio P/L:', { timeout: 15_000 });
  const text = await outputText(page);
  expect(text).toContain('Side');
  expect(text).toContain('short');
  expect(text).toContain('Break-even');
  const aaplIndex = text.indexOf('AAPL');
  const gmeIndex = text.indexOf('GME');
  const tslaIndex = text.indexOf('TSLA');
  expect(aaplIndex).toBeGreaterThan(-1);
  expect(gmeIndex).toBeGreaterThan(-1);
  expect(tslaIndex).toBeGreaterThan(-1);
  expect(aaplIndex).toBeLessThan(gmeIndex);
  expect(gmeIndex).toBeLessThan(tslaIndex);
});

test('portfolio-pnl page renders alternate currency prefixes and percentage-fee boundary', async ({ page }) => {
  await page.goto('/tools/portfolio-pnl/');
  await page.fill('#in-positions', 'BTC, 0.25, 40000, 60000');
  await page.fill('#in-fee_percent', '5');
  await page.fill('#in-currency', 'USD ');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('USD10,000.00', { timeout: 15_000 });
  await expect(out).toContainText('Fees', { timeout: 15_000 });
  const text = await outputText(page);
  expect(text).toContain('USD1,250.00');
});

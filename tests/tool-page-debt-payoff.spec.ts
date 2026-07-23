import { test, expect } from './fixtures';

const SAMPLE_DEBTS = 'Visa, 2500, 19.99, 75\nCar Loan, 8000, 6.5, 200\nStore Card, 600, 24.99, 25';

async function parseOutput(page: any) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"payoff_order"', { timeout: 15000 });
  const text = await out.textContent();
  expect(text).toBeTruthy();
  return JSON.parse(text!);
}

test('debt-payoff page returns a real snowball payoff plan', async ({ page }) => {
  await page.goto('/tools/debt-payoff/');
  await page.fill('#in-debts', SAMPLE_DEBTS);
  await page.selectOption('#in-method', 'snowball');
  await page.fill('#in-extra_payment', '300');
  await page.fill('#in-start_date', '2026-01-01');

  const result = await parseOutput(page);
  expect(result.method).toBe('snowball');
  expect(result.start_date).toBe('2026-01-01');
  expect(result.debt_free_date).toMatch(/^20\d\d-\d\d-\d\d$/);
  expect(result.months).toBeGreaterThan(0);
  expect(result.total_interest).toBeGreaterThan(0);
  expect(result.payoff_order.map((d: any) => d.name)).toEqual(['Store Card', 'Visa', 'Car Loan']);
  expect(result.minimum_only.feasible).toBe(true);
  expect(result.interest_saved_vs_minimum).toBeGreaterThan(0);
  expect(result.comparison.snowball.method).toBe('snowball');
  expect(result.comparison.avalanche.method).toBe('avalanche');
});

test('debt-payoff deep-link prefills avalanche params and computes', async ({ page }) => {
  const qs =
    '?debts=' + encodeURIComponent(SAMPLE_DEBTS) +
    '&method=avalanche' +
    '&extra_payment=300' +
    '&start_date=2026-01-01';
  await page.goto('/tools/debt-payoff/' + qs);

  await expect(page.locator('#in-debts')).toHaveValue(SAMPLE_DEBTS, { timeout: 15000 });
  await expect(page.locator('#in-method')).toHaveValue('avalanche');
  await expect(page.locator('#in-extra_payment')).toHaveValue('300');
  await expect(page.locator('#in-start_date')).toHaveValue('2026-01-01');

  const result = await parseOutput(page);
  expect(result.method).toBe('avalanche');
  expect(result.payoff_order[0].name).toBe('Store Card');
  expect(result.comparison.recommended).toMatch(/snowball|avalanche/);
});

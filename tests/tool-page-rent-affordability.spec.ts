import { test, expect } from './fixtures';

async function outputJson(page: import('@playwright/test').Page) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('recommended_max_rent', { timeout: 15_000 });
  return JSON.parse((await out.textContent()) ?? '{}');
}

test('rent-affordability computes the classic 30% rule with exact values', async ({ page }) => {
  await page.goto('/tools/rent-affordability/');
  await page.fill('#in-income', '60000');
  await page.selectOption('#in-income_period', 'annual');
  await page.selectOption('#in-income_type', 'gross');
  await page.fill('#in-rent_to_income_ratio', '30');
  await page.fill('#in-monthly_debts', '0');
  await page.fill('#in-max_dti_ratio', '36');

  const json = await outputJson(page);
  expect(json.gross_monthly_income).toBe(5000);
  expect(json.max_affordable_rent).toBe(1500);
  expect(json.debt_adjusted_max_rent).toBe(1800);
  expect(json.recommended_max_rent).toBe(1500);
  expect(json.guideline_range).toEqual({ conservative: 1250, moderate: 1500, aggressive: 1750 });
  expect(json.summary).toContain('$1,500.00 /mo');
});

test('rent-affordability debt adjustment can lower recommended rent', async ({ page }) => {
  await page.goto('/tools/rent-affordability/');
  await page.fill('#in-income', '6000');
  await page.selectOption('#in-income_period', 'monthly');
  await page.fill('#in-rent_to_income_ratio', '30');
  await page.fill('#in-monthly_debts', '800');
  await page.fill('#in-max_dti_ratio', '36');

  const json = await outputJson(page);
  expect(json.max_affordable_rent).toBe(1800);
  expect(json.debt_adjusted_max_rent).toBe(1360);
  expect(json.recommended_max_rent).toBe(1360);
  expect(json.remaining_after_rent_and_debts).toBe(3840);
  expect(json.summary).toContain('after $800.00 /mo of debts');
});

test('rent-affordability covers enum choices, custom ratio, currency, and decimals', async ({ page }) => {
  await page.goto('/tools/rent-affordability/');
  await page.fill('#in-income', '4000');
  await page.selectOption('#in-income_period', 'monthly');
  await page.selectOption('#in-income_type', 'net');
  await page.fill('#in-tax_rate_percent', '25');
  await page.fill('#in-rent_to_income_ratio', '40');
  await page.fill('#in-currency', '£');
  await page.fill('#in-decimals', '0');

  const json = await outputJson(page);
  expect(json.gross_monthly_income).toBe(5333);
  expect(json.max_affordable_rent).toBe(2133);
  expect(json.recommended_max_rent).toBe(1920);
  expect(json.summary).toContain('£2,133.33 /mo');
});

test('rent-affordability deep-links defaults and boundary ratio', async ({ page }) => {
  const qs = new URLSearchParams({
    income: '60000',
    income_period: 'annual',
    income_type: 'gross',
    tax_rate_percent: '25',
    rent_to_income_ratio: '50',
    monthly_debts: '0',
    max_dti_ratio: '60',
    currency: '$',
    decimals: '2',
  });
  await page.goto(`/tools/rent-affordability/?${qs.toString()}`);

  await expect(page.locator('#in-income')).toHaveValue('60000');
  await expect(page.locator('#in-income_period')).toHaveValue('annual');
  await expect(page.locator('#in-rent_to_income_ratio')).toHaveValue('50');

  const json = await outputJson(page);
  expect(json.max_affordable_rent).toBe(2500);
  expect(json.debt_adjusted_max_rent).toBe(3000);
  expect(json.recommended_max_rent).toBe(2500);
});

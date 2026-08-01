import { test, expect } from './fixtures';

async function outputJson(page: import('@playwright/test').Page) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('net_worth_difference', { timeout: 15_000 });
  return JSON.parse((await out.textContent()) ?? '{}');
}

test('rent-vs-buy computes the default 10-year scenario with exact values', async ({ page }) => {
  await page.goto('/tools/rent-vs-buy/');
  await page.fill('#in-home_price', '400000');
  await page.fill('#in-down_payment_percent', '20');
  await page.fill('#in-mortgage_rate_percent', '6.5');
  await page.fill('#in-loan_term_years', '30');
  await page.fill('#in-monthly_rent', '2000');
  await page.fill('#in-years', '10');

  const json = await outputJson(page);
  expect(json.loan_amount).toBe(320000);
  expect(json.down_payment).toBe(80000);
  expect(json.total_upfront_cost).toBe(92000);
  expect(json.monthly_principal_interest).toBe(2023);
  expect(json.buy_net_worth).toBe(234029);
  expect(json.rent_net_worth).toBe(265943);
  expect(json.net_worth_difference).toBe(-31914);
  expect(json.verdict).toBe('rent');
  expect(json.break_even_year).toBeNull();
  expect(json.yearly).toHaveLength(10);
  expect(json.summary).toContain('Over 10 years, renting wins by $31,914.43');
});

test('rent-vs-buy long 20-year stay lets buying win with a break-even year', async ({ page }) => {
  await page.goto('/tools/rent-vs-buy/');
  await page.fill('#in-home_price', '400000');
  await page.fill('#in-monthly_rent', '2000');
  await page.fill('#in-years', '20');

  const json = await outputJson(page);
  expect(json.buy_net_worth).toBe(500969);
  expect(json.rent_net_worth).toBe(481826);
  expect(json.net_worth_difference).toBe(19143);
  expect(json.verdict).toBe('buy');
  expect(json.break_even_year).toBe(18);
  expect(json.yearly).toHaveLength(20);
});

test('rent-vs-buy applies currency and decimals with a break-even summary', async ({ page }) => {
  await page.goto('/tools/rent-vs-buy/');
  await page.fill('#in-monthly_rent', '2500');
  await page.fill('#in-currency', '£');
  await page.fill('#in-decimals', '2');

  const json = await outputJson(page);
  expect(json.monthly_principal_interest).toBe(2022.62);
  expect(json.net_worth_difference).toBe(55521.01);
  expect(json.verdict).toBe('buy');
  expect(json.break_even_year).toBe(6);
  expect(json.summary).toContain('£55,521.01');
  expect(json.summary).toContain('Buying pulls ahead around year 6');
});

test('rent-vs-buy deep-links a short 3-year stay where renting wins', async ({ page }) => {
  const qs = new URLSearchParams({
    home_price: '400000',
    down_payment_percent: '20',
    mortgage_rate_percent: '6.5',
    loan_term_years: '30',
    monthly_rent: '2000',
    years: '3',
    home_appreciation_percent: '3',
    rent_growth_percent: '3',
    investment_return_percent: '5',
    currency: '$',
    decimals: '0',
  });
  await page.goto(`/tools/rent-vs-buy/?${qs.toString()}`);

  await expect(page.locator('#in-home_price')).toHaveValue('400000');
  await expect(page.locator('#in-years')).toHaveValue('3');
  await expect(page.locator('#in-monthly_rent')).toHaveValue('2000');

  const json = await outputJson(page);
  expect(json.verdict).toBe('rent');
  expect(json.net_worth_difference).toBe(-37760);
  expect(json.break_even_year).toBeNull();
  expect(json.yearly).toHaveLength(3);
  expect(json.summary).toContain('Over 3 years, renting wins by $37,759.78');
});

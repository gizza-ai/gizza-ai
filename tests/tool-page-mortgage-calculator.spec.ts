import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('mortgage-calculator page — classic principal and interest output', async ({ page }) => {
  await page.goto('/tools/mortgage-calculator/');
  await page.fill('#in-home_price', '250000');
  await page.fill('#in-down_payment', '50000');
  await page.fill('#in-loan_years', '30');
  await page.fill('#in-annual_interest_rate_percent', '6');
  await page.fill('#in-annual_property_tax', '0');
  await page.fill('#in-annual_insurance', '0');
  await page.fill('#in-monthly_hoa', '0');
  await page.fill('#in-extra_monthly_payment', '0');
  await page.fill('#in-decimals', '2');
  await expect(page.locator('#tool-output')).toContainText('"monthly_principal_interest": 1199.1', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('"loan_amount": 200000');
  expect(out).toContain('"payoff_months": 360');
  expect(out).toContain('"monthly_payment": 1199.1');
});

test('mortgage-calculator page — taxes insurance and HOA are included', async ({ page }) => {
  await page.goto('/tools/mortgage-calculator/');
  await page.fill('#in-home_price', '250000');
  await page.fill('#in-down_payment', '50000');
  await page.fill('#in-loan_years', '30');
  await page.fill('#in-annual_interest_rate_percent', '6');
  await page.fill('#in-annual_property_tax', '3600');
  await page.fill('#in-annual_insurance', '1200');
  await page.fill('#in-monthly_hoa', '150');
  await page.fill('#in-extra_monthly_payment', '0');
  await page.fill('#in-decimals', '2');
  await expect(page.locator('#tool-output')).toContainText('"monthly_taxes": 300', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('"monthly_insurance": 100');
  expect(out).toContain('"monthly_hoa": 150');
  expect(out).toContain('"monthly_payment": 1749.1');
});

test('mortgage-calculator page — extra payment shortens payoff', async ({ page }) => {
  await page.goto('/tools/mortgage-calculator/');
  await page.fill('#in-home_price', '12000');
  await page.fill('#in-down_payment', '0');
  await page.fill('#in-loan_years', '10');
  await page.fill('#in-annual_interest_rate_percent', '0');
  await page.fill('#in-annual_property_tax', '0');
  await page.fill('#in-annual_insurance', '0');
  await page.fill('#in-monthly_hoa', '0');
  await page.fill('#in-extra_monthly_payment', '100');
  await page.fill('#in-decimals', '2');
  await expect(page.locator('#tool-output')).toContainText('"payoff_months": 60', { timeout: 15000 });
  expect(await outputText(page)).toContain('"total_interest": 0');
});

test('mortgage-calculator page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto('/tools/mortgage-calculator/?home_price=250000&down_payment=50000&loan_years=30&annual_interest_rate_percent=6&annual_property_tax=0&annual_insurance=0&monthly_hoa=0&extra_monthly_payment=0&decimals=2');
  await expect(page.locator('#in-home_price')).toHaveValue('250000', { timeout: 15000 });
  await expect(page.locator('#in-loan_years')).toHaveValue('30');
  await expect(page.locator('#tool-output')).toContainText('"monthly_principal_interest": 1199.1', { timeout: 15000 });
});

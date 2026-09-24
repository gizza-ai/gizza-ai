import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('amortization-schedule page — exact annual CSV output', async ({ page }) => {
  await page.goto('/tools/amortization-schedule/');
  await page.fill('#in-loan_amount', '1000');
  await page.fill('#in-annual_interest_rate_percent', '12');
  await page.fill('#in-loan_years', '3');
  await page.fill('#in-loan_months', '0');
  await page.selectOption('#in-payment_frequency', 'annual');
  await page.fill('#in-start_date', '2026-01-01');
  await page.fill('#in-extra_payment', '0');
  await page.fill('#in-extra_one_time', '0');
  await page.fill('#in-extra_one_time_period', '1');
  await page.selectOption('#in-schedule_view', 'period');
  await page.selectOption('#in-format', 'csv');
  await page.fill('#in-currency_symbol', '$');
  await expect(page.locator('#tool-output')).toContainText('period,date,payment,principal,interest,extra,balance', { timeout: 15000 });
  expect(await outputText(page)).toContain('1,2026-01-01,416.35,296.35,120.00,0.00,703.65');
  expect(await outputText(page)).toContain('3,2028-01-01,416.35,371.74,44.61,0.00,0.00');
});

test('amortization-schedule page — extra payment reports savings', async ({ page }) => {
  await page.goto('/tools/amortization-schedule/');
  await page.fill('#in-loan_amount', '300000');
  await page.fill('#in-annual_interest_rate_percent', '6');
  await page.fill('#in-loan_years', '30');
  await page.fill('#in-loan_months', '0');
  await page.selectOption('#in-payment_frequency', 'monthly');
  await page.fill('#in-start_date', '2026-01-01');
  await page.fill('#in-extra_payment', '200');
  await page.fill('#in-extra_one_time', '0');
  await page.fill('#in-extra_one_time_period', '1');
  await page.selectOption('#in-schedule_view', 'period');
  await page.selectOption('#in-format', 'table');
  await page.fill('#in-currency_symbol', '$');
  await expect(page.locator('#tool-output')).toContainText('Interest saved', { timeout: 15000 });
  const out = await outputText(page);
  expect(out).toContain('$91,173.87');
  expect(out).toContain('Payments saved');
  expect(out).toContain('cut 81 payments');
});

test('amortization-schedule page — query-param deep-link prefills and runs', async ({ page }) => {
  await page.goto('/tools/amortization-schedule/?loan_amount=1000&annual_interest_rate_percent=12&loan_years=1&loan_months=0&payment_frequency=monthly&start_date=2026-01-01&extra_payment=0&extra_one_time=0&extra_one_time_period=1&schedule_view=period&format=csv&currency_symbol=%24');
  await expect(page.locator('#in-loan_amount')).toHaveValue('1000', { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toContainText('1,2026-01-01,88.85,78.85,10.00,0.00,921.15', { timeout: 15000 });
});

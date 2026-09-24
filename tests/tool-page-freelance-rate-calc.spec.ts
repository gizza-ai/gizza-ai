import { test, expect } from './fixtures';

async function runWasm(
  page: any,
  targetIncome = '80000',
  businessExpenses = '3000',
  healthInsurance = '0',
  retirement = '0',
  taxRate = '30',
  taxBasis = 'income_only',
  hoursPerWeek = '40',
  daysPerWeek = '5',
  hoursPerDay = '8',
  weeksPerYear = '52',
  vacationWeeks = '4',
  holidays = '10',
  sickDays = '5',
  billablePercent = '70',
  bufferPercent = '0',
  currentRate = '0',
  projectHours = '0',
  complexity = 'standard',
  currency = '$',
  decimals = '2',
  format = 'json',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/freelance-rate-calc/gizza_ai_freelance_rate_calc_web.js');
    await mod.default('/tools/freelance-rate-calc/gizza_ai_freelance_rate_calc_web_bg.wasm');
    return mod.run(
      args.targetIncome,
      args.businessExpenses,
      args.healthInsurance,
      args.retirement,
      args.taxRate,
      args.taxBasis,
      args.hoursPerWeek,
      args.daysPerWeek,
      args.hoursPerDay,
      args.weeksPerYear,
      args.vacationWeeks,
      args.holidays,
      args.sickDays,
      args.billablePercent,
      args.bufferPercent,
      args.currentRate,
      args.projectHours,
      args.complexity,
      args.currency,
      args.decimals,
      args.format,
    );
  }, { targetIncome, businessExpenses, healthInsurance, retirement, taxRate, taxBasis, hoursPerWeek, daysPerWeek, hoursPerDay, weeksPerYear, vacationWeeks, holidays, sickDays, billablePercent, bufferPercent, currentRate, projectHours, complexity, currency, decimals, format });
}

const BASE_JSON = `{
  "hourly_rate": 93.08,
  "day_rate": 744.67,
  "week_rate": 2606.35,
  "month_rate": 9773.81,
  "worked_hour_rate": 65.16,
  "break_even_hourly": 93.08,
  "buffer_per_hour": 0,
  "required_revenue": 117285.71,
  "target_income": 80000,
  "income_tax": 34285.71,
  "self_employment_tax": 0,
  "total_tax": 34285.71,
  "effective_tax_percent": 30,
  "total_costs": 3000,
  "business_expenses": 3000,
  "health_insurance": 0,
  "retirement": 0,
  "weeks_available": 45,
  "work_days": 225,
  "work_hours": 1800,
  "billable_hours": 1260,
  "billable_days": 157.5
}`;

test('freelance-rate-calc wasm computes the baseline JSON exactly', async ({ page }) => {
  await page.goto('/tools/freelance-rate-calc/');
  await page.waitForSelector('#in-target_income');

  await expect(runWasm(page)).resolves.toBe(BASE_JSON);
});

test('freelance-rate-calc wasm covers self-employment tax, buffer, current rate and project quote', async ({ page }) => {
  await page.goto('/tools/freelance-rate-calc/');
  await page.waitForSelector('#in-target_income');

  const out = await runWasm(
    page,
    '120000',
    '12000',
    '9000',
    '15000',
    '24',
    'self_employment',
    '40',
    '5',
    '8',
    '52',
    '5',
    '10',
    '5',
    '65',
    '10',
    '125',
    '40',
    'rush',
    '$',
    '2',
    'json',
  );
  expect(out).toContain('"self_employment_tax":');
  expect(out).toContain('"buffer_per_hour":');
  expect(out).toContain('"current_rate": {');
  expect(out).toContain('"quote": {');
  expect(out).toContain('"complexity": "Rush"');
});

test('freelance-rate-calc page renders exact output and honors controls', async ({ page }) => {
  await page.goto('/tools/freelance-rate-calc/');
  await page.locator('#in-target_income').fill('80000');
  await page.locator('#in-business_expenses').fill('3000');
  await page.locator('#in-tax_rate').fill('30');
  await page.locator('#in-billable_percent').fill('70');
  await page.locator('#in-format').selectOption('json');
  await expect(page.locator('#tool-output')).toHaveText(BASE_JSON, { timeout: 15_000 });
});

test('freelance-rate-calc deep-link prefills controls and runs exact output', async ({ page }) => {
  const params = new URLSearchParams({
    target_income: '80000',
    business_expenses: '3000',
    tax_rate: '30',
    billable_percent: '70',
    format: 'json',
  });

  await page.goto(`/tools/freelance-rate-calc/?${params.toString()}`);
  await expect(page.locator('#in-target_income')).toHaveValue('80000', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toHaveText(BASE_JSON, { timeout: 15_000 });

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool freelance-rate-calc');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

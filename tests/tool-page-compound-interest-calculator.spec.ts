import { test, expect } from './fixtures';

async function parseOutput(page: any) {
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"future_value"', { timeout: 15000 });
  const text = await out.textContent();
  expect(text).toBeTruthy();
  return JSON.parse(text!);
}

test('compound-interest page computes the default monthly-compounded lump sum', async ({ page }) => {
  await page.goto('/tools/compound-interest-calculator/');
  await page.fill('#in-principal', '1000');
  await page.fill('#in-annual_rate', '5');
  await page.fill('#in-years', '10');
  await page.fill('#in-months', '0');
  await page.selectOption('#in-compounding', 'monthly');
  await page.fill('#in-contribution', '0');

  const r = await parseOutput(page);
  expect(r.future_value).toBe(1647.01);
  expect(r.principal).toBe(1000.0);
  expect(r.total_contributions).toBe(0.0);
  expect(r.total_interest).toBe(647.01);
  expect(r.effective_annual_rate).toBe(5.1162);
  expect(r.years_to_double).toBeCloseTo(13.892, 2);
  expect(r.summary).toContain('grows to 1,647.01');
  expect(r.schedule.length).toBe(10);
  expect(r.schedule[r.schedule.length - 1].year).toBe(10.0);
});

test('compound-interest exercises every compounding value form', async ({ page }) => {
  await page.goto('/tools/compound-interest-calculator/');
  await page.fill('#in-principal', '1000');
  await page.fill('#in-annual_rate', '5');
  await page.fill('#in-years', '10');
  await page.fill('#in-months', '0');
  await page.fill('#in-contribution', '0');

  const cases: Array<[string, number, number]> = [
    ['annually', 1628.89, 5.0],
    ['semiannually', 1638.62, 5.0625],
    ['quarterly', 1643.62, 5.0945],
    ['monthly', 1647.01, 5.1162],
    ['weekly', 1648.33, 5.1246],
    ['daily', 1648.66, 5.1267],
    ['continuously', 1648.72, 5.1271],
  ];
  for (const [freq, fv, ear] of cases) {
    await page.selectOption('#in-compounding', freq);
    const r = await parseOutput(page);
    expect(r.effective_annual_rate, freq).toBe(ear);
    expect(r.future_value, freq).toBeCloseTo(fv, 2);
  }
});

test('compound-interest values regular contributions and annuity-due timing', async ({ page }) => {
  await page.goto('/tools/compound-interest-calculator/');
  await page.fill('#in-principal', '0');
  await page.fill('#in-annual_rate', '6');
  await page.fill('#in-years', '1');
  await page.fill('#in-months', '0');
  await page.selectOption('#in-compounding', 'monthly');
  await page.fill('#in-contribution', '100');
  await page.selectOption('#in-contribution_frequency', 'monthly');

  await page.selectOption('#in-contribution_timing', 'end');
  const end = await parseOutput(page);
  expect(end.future_value).toBe(1233.56);
  expect(end.total_contributions).toBe(1200.0);
  expect(end.total_interest).toBe(33.56);

  await page.selectOption('#in-contribution_timing', 'start');
  const start = await parseOutput(page);
  expect(start.future_value).toBeGreaterThan(end.future_value);

  // Non-default contribution frequency form (bi-weekly, 26x/yr).
  await page.selectOption('#in-contribution_timing', 'end');
  await page.selectOption('#in-contribution_frequency', 'biweekly');
  const biweekly = await parseOutput(page);
  expect(biweekly.total_contributions).toBe(2600.0);
});

test('compound-interest enforces the term cap at and one over the boundary', async ({ page }) => {
  await page.goto('/tools/compound-interest-calculator/');
  await page.fill('#in-principal', '1000');
  await page.fill('#in-annual_rate', '5');
  await page.fill('#in-months', '0');
  await page.fill('#in-contribution', '0');

  await page.fill('#in-years', '1000');
  const ok = await parseOutput(page);
  expect(ok.years).toBe(1000.0);

  await page.fill('#in-years', '1001');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('at most 1000 years', { timeout: 15000 });
});

test('compound-interest deep-link prefills params and computes', async ({ page }) => {
  const qs =
    '?principal=5000&annual_rate=7&years=10&months=0' +
    '&compounding=continuously&contribution=0' +
    '&contribution_frequency=monthly&contribution_timing=end';
  await page.goto('/tools/compound-interest-calculator/' + qs);

  await expect(page.locator('#in-principal')).toHaveValue('5000', { timeout: 15000 });
  await expect(page.locator('#in-annual_rate')).toHaveValue('7');
  await expect(page.locator('#in-compounding')).toHaveValue('continuously');

  const r = await parseOutput(page);
  // 5000 * e^0.7 = 10068.76...
  expect(r.future_value).toBeCloseTo(10068.76, 1);
  expect(r.effective_annual_rate).toBe(7.2508);
});

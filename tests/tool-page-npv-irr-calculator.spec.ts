import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('npv-irr-calculator computes the worked annual project', async ({ page }) => {
  await page.goto('/tools/npv-irr-calculator/');
  await setField(page, '#in-cash_flows', '-100000, 30000, 30000, 30000, 30000, 30000');
  await setField(page, '#in-discount_rate', '8');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('NPV: $19,781.30', { timeout: 15_000 });
  await expect(out).toContainText('IRR: +15.24% per year');
  await expect(out).toContainText('MIRR: +11.97% per year');
  await expect(out).toContainText('Profitability index: 1.1978');
  await expect(out).toContainText('5         $30,000.00  0.680583     $20,417.50     $19,781.30');
});

test('npv-irr-calculator honors deep-linked monthly JSON parameters', async ({ page }) => {
  const params = new URLSearchParams({
    cash_flows: '12x1000',
    initial_investment: '11000',
    discount_rate: '12',
    period: 'monthly',
    timing: 'end',
    decimals: '2',
    currency: '$',
    output: 'json',
  });
  await page.goto(`/tools/npv-irr-calculator/?${params.toString()}`);

  await expect(page.locator('#in-period')).toHaveValue('monthly');
  await expect(page.locator('#in-output')).toHaveValue('json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"period": "monthly"', { timeout: 15_000 });
  await expect(out).toContainText('"periods_per_year": 12.0');
  await expect(out).toContainText('"period_discount_rate": 0.01');
  await expect(out).toContainText('"count": 13');
});

test('npv-irr-calculator covers enum choices and cap boundary', async ({ page }) => {
  await page.goto('/tools/npv-irr-calculator/');
  await setField(page, '#in-cash_flows', '-1000, 600, 600');
  await page.selectOption('#in-timing', 'begin');
  await page.selectOption('#in-period', 'annual');
  await setField(page, '#in-discount_rate', '10');
  await setField(page, '#in-decimals', '2');
  await setField(page, '#in-currency', '€');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toHaveText(
    'period,cash_flow,discount_periods,discount_factor,present_value,cumulative_pv,cumulative_cash_flow\n0,-1000,0,1.0000000000,-1000.000000,-1000.000000,-1000.000000\n1,600,0,1.0000000000,600.000000,-400.000000,-400.000000\n2,600,1,0.9090909091,545.454545,145.454545,200.000000',
    { timeout: 15_000 },
  );

  await setField(page, '#in-cash_flows', `-1000, ${1199}x1`);
  await page.selectOption('#in-output', 'summary');
  await setField(page, '#in-decimals', '0');
  await expect(page.locator('#tool-output')).toContainText('Cash flows: 1200 periods (0 to 1199)', { timeout: 15_000 });

  await page.selectOption('#in-period', 'quarterly');
  await expect(page.locator('#tool-output')).toContainText('4 periods per year', { timeout: 15_000 });

  await page.selectOption('#in-period', 'semiannual');
  await expect(page.locator('#tool-output')).toContainText('2 periods per year', { timeout: 15_000 });

  await page.selectOption('#in-period', 'weekly');
  await expect(page.locator('#tool-output')).toContainText('52 periods per year', { timeout: 15_000 });

  await setField(page, '#in-cash_flows', `-1000, ${1200}x1`);
  await expect(page.locator('#tool-output')).toContainText('the maximum is 1200', { timeout: 15_000 });
});

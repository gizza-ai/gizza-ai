import { test, expect } from './fixtures';

const values = `2019,100000
2020,112000
2021,128500
2022,141000
2023,150000`;

const summaryOutput = `CAGR: +10.67% per year

start (2019): 100,000
end (2023): 150,000
elapsed: 4 years (5 data points, 4 periods, from the series spacing)
total growth: +50.00%  (1.5x, 50,000 absolute)
compound growth per period: +10.67%
average period growth (arithmetic): +10.71%
best period: +14.73% at 2021
worst period: +6.38% at 2023
doubling time at this CAGR: 6.84 years

period    value  change   growth   index
----------------------------------------
2019    100,000       —        —  100.00
2020    112,000  12,000  +12.00%  112.00
2021    128,500  16,500  +14.73%  128.50
2022    141,000  12,500   +9.73%  141.00
2023    150,000   9,000   +6.38%  150.00

CAGR is the constant annual rate that turns the first value into the last one over 4 annual periods — it smooths away everything in between, so it is not the average of the period growth rates and says nothing about volatility. Educational only, not financial advice.`;

test('cagr-calculator page emits exact summary output', async ({ page }) => {
  await page.goto('/tools/cagr-calculator/');
  await page.fill('#in-values', values);
  await page.selectOption('#in-period', 'annual');
  await page.fill('#in-decimals', '2');
  await page.selectOption('#in-output', 'summary');

  await expect(page.locator('#tool-output')).toHaveText(summaryOutput, { timeout: 15_000 });
});

test('cagr-calculator honours deep-link date range, header and csv output', async ({ page }) => {
  const qs =
    '?values=' + encodeURIComponent(`month,users
2024-01,12000
2024-02,13100
2024-03,14250
2024-04,15900`) +
    '&period=monthly' +
    '&start_date=2024-01-31' +
    '&end_date=2024-04-30' +
    '&has_header=true' +
    '&decimals=2' +
    '&output=csv';
  await page.goto('/tools/cagr-calculator/' + qs);

  await expect(page.locator('#in-period')).toHaveValue('monthly', { timeout: 15_000 });
  await expect(page.locator('#in-start_date')).toHaveValue('2024-01-31');
  await expect(page.locator('#in-end_date')).toHaveValue('2024-04-30');
  await expect(page.locator('#in-has_header')).toBeChecked();
  await expect(page.locator('#in-output')).toHaveValue('csv');

  await expect(page.locator('#tool-output')).toHaveText(
    `period,value,change,growth_pct,index
2024-01,12000,,,100
2024-02,13100,1100,9.17,109.17
2024-03,14250,1150,8.78,118.75
2024-04,15900,1650,11.58,132.50`,
    { timeout: 15_000 },
  );
});

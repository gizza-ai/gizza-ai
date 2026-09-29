import { test, expect } from './fixtures';

const SAMPLE = `Revenue: 1,200,000
COGS: 720,000
Operating expenses: 300,000
Depreciation and amortization: 40,000
Interest expense: 20,000
Taxes: 40,000
Net income: 120,000
Cash: 90,000
Accounts receivable: 150,000
Inventory: 180,000
Total current assets: 420,000
Fixed assets: 580,000
Accounts payable: 110,000
Short term debt: 60,000
Total current liabilities: 170,000
Long term debt: 330,000
Retained earnings: 200,000
Total equity: 500,000`;

const PRIOR = `Revenue: 1,000,000
Net income: 80,000
Total assets: 800,000
Total equity: 400,000`;

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('financial-ratio-analyzer computes a statement summary', async ({ page }) => {
  await page.goto('/tools/financial-ratio-analyzer/');
  await setField(page, '#in-figures', SAMPLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Financial ratio analysis: 34 ratios', { timeout: 15_000 });
  await expect(out).toContainText('Current ratio                          2.47x');
  await expect(out).toContainText('Quick ratio (acid test)                1.41x');
  await expect(out).toContainText('Debt to equity                         1.00x');
  await expect(out).toContainText('Gross margin                          40.00%');
  await expect(out).toContainText('Return on equity (ROE)                24.00%');
  await expect(out).toContainText('Educational arithmetic only, not financial, investment, tax or accounting advice.');
});

test('financial-ratio-analyzer honors deep-linked prior-period JSON parameters', async ({ page }) => {
  const params = new URLSearchParams({
    figures: SAMPLE,
    prior_figures: PRIOR,
    groups: 'returns',
    basis: 'average',
    days_in_period: '365',
    benchmarks: 'true',
    decimals: '2',
    currency: '$',
    output: 'json',
  });
  await page.goto(`/tools/financial-ratio-analyzer/?${params.toString()}`);

  await expect(page.locator('#in-groups')).toHaveValue('returns');
  await expect(page.locator('#in-output')).toHaveValue('json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"basis": "average"', { timeout: 15_000 });
  await expect(out).toContainText('"groups": "returns"');
  await expect(out).toContainText('"key": "return_on_assets"');
  await expect(out).toContainText('"value": 13.333333333333334');
});

test('financial-ratio-analyzer covers enum choices, checkbox off and cap boundary', async ({ page }) => {
  await page.goto('/tools/financial-ratio-analyzer/');
  await setField(page, '#in-figures', SAMPLE);
  await page.selectOption('#in-groups', 'liquidity');
  await page.selectOption('#in-basis', 'ending');
  await setField(page, '#in-days_in_period', '360');
  await page.uncheck('#in-benchmarks');
  await setField(page, '#in-decimals', '1');
  await setField(page, '#in-currency', '€');
  await page.selectOption('#in-output', 'csv');

  await expect(page.locator('#tool-output')).toContainText(
    'group,key,label,value,unit,prior,change,benchmark_low,benchmark_high,status,formula,note',
    { timeout: 15_000 },
  );
  await expect(page.locator('#tool-output')).toContainText('liquidity,current_ratio,Current ratio,2.4705882352941178');

  await page.selectOption('#in-output', 'summary');
  await expect(page.locator('#tool-output')).not.toContainText('Health score');
  await expect(page.locator('#tool-output')).not.toContainText('target');

  await page.selectOption('#in-groups', 'market');
  await setField(page, '#in-figures', `${SAMPLE}\nShares outstanding: 100,000\nShare price: 24`);
  await expect(page.locator('#tool-output')).toContainText('Price / earnings', { timeout: 15_000 });

  const maxLines = Array.from({ length: 400 }, () => 'Revenue: 1000').join('\n');
  await setField(page, '#in-figures', maxLines);
  await expect(page.locator('#tool-output')).not.toContainText('the maximum is 400', { timeout: 15_000 });
  await setField(page, '#in-figures', `${maxLines}\nRevenue extra: 1000`);
  await expect(page.locator('#tool-output')).toContainText('the maximum is 400', { timeout: 15_000 });
});

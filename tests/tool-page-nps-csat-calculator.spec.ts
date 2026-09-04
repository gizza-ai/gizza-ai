import { test, expect } from './fixtures';

async function setField(page: import('@playwright/test').Page, id: string, value: string) {
  await page.locator(id).evaluate((el, v) => {
    (el as HTMLInputElement | HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('nps-csat-calculator computes NPS from raw ratings', async ({ page }) => {
  await page.goto('/tools/nps-csat-calculator/');
  await setField(page, '#in-ratings', 'score\n10\n9\n8\n7\n6\n10\n0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Net Promoter Score (NPS)', { timeout: 15_000 });
  await expect(out).toContainText('NPS                       14.3');
  await expect(out).toContainText('Promoters    (9-10)         3   42.9%');
  await expect(out).toContainText('Detractors   (0-6)          2   28.6%');
  await expect(out).toContainText('Next tier: Great (30+)');
});

test('nps-csat-calculator honors deep-linked CSAT JSON parameters', async ({ page }) => {
  const params = new URLSearchParams({
    ratings: '5,5,4,4,4,3,2,1',
    metric: 'csat',
    input: 'values',
    scale: '1-5',
    threshold: '4',
    confidence: '90',
    decimals: '1',
    distribution: 'true',
    format: 'json',
  });
  await page.goto(`/tools/nps-csat-calculator/?${params.toString()}`);

  await expect(page.locator('#in-metric')).toHaveValue('csat');
  await expect(page.locator('#in-confidence')).toHaveValue('90');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"metric": "csat"', { timeout: 15_000 });
  await expect(out).toContainText('"score": 62.5');
  await expect(out).toContainText('"confidence": { "level": 90, "margin": 28.2');
});

test('nps-csat-calculator covers CES CSV, checkbox and boundary controls', async ({ page }) => {
  await page.goto('/tools/nps-csat-calculator/');
  await setField(page, '#in-ratings', '7 6 6 5 5 4 3 2');
  await page.selectOption('#in-metric', 'ces');
  await page.selectOption('#in-scale', '1-7');
  await setField(page, '#in-threshold', '5');
  await page.selectOption('#in-confidence', 'none');
  await setField(page, '#in-decimals', '2');
  await page.uncheck('#in-distribution');
  await page.selectOption('#in-format', 'csv');

  await expect(page.locator('#tool-output')).toHaveText(
    'section,label,value,percent\nscore,ces,4.75,\nscore,rating,Moderate effort,\nsample,responses,8,\nsample,skipped,0,\nsample,mean,4.75,\nsample,std_dev,1.67,\nsample,threshold,5,\nband,easy 5-7,5,62.50\nband,neutral 4,1,12.50\nband,difficult 1-3,2,25.00\n',
    { timeout: 15_000 },
  );

  await setField(page, '#in-ratings', '10:1');
  await page.selectOption('#in-metric', 'nps');
  await page.selectOption('#in-input', 'counts');
  await page.selectOption('#in-scale', '0-10');
  await page.selectOption('#in-confidence', 'none');
  await setField(page, '#in-decimals', '0');
  await page.selectOption('#in-format', 'report');
  await page.check('#in-distribution');
  await expect(page.locator('#tool-output')).toContainText('NPS                       100', { timeout: 15_000 });
});

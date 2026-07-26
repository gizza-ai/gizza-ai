import { test, expect } from './fixtures';

const CSV = 'city,churn\nA,1\nB,1\nA,0\nB,1\nC,1\nB,0';

// A = (1+0)/2 = 0.5, B = (1+1+0)/3 = 0.6667, C = 1/1 = 1.0 (default 4 decimals)
const REPLACE_OUTPUT = `city,churn
0.5000,1
0.6667,1
0.5000,0
0.6667,1
1.0000,1
0.6667,0`;

// prior = 4/6 = 0.66667, m = 3: A=(1+2)/5=0.6, B=(2+2)/6=0.6667, C=(1+2)/4=0.75
const APPEND_SMOOTHED_OUTPUT = `city,churn,city_target_enc
A,1,0.6000
B,1,0.6667
A,0,0.6000
B,1,0.6667
C,1,0.7500
B,0,0.6667`;

test('target-mean-encoder replaces a category column with its target mean', async ({ page }) => {
  await page.goto('/tools/target-mean-encoder/');
  await page.fill('#in-data', CSV);
  await page.fill('#in-category', 'city');
  await page.fill('#in-target', 'churn');
  await page.selectOption('#in-output', 'replace');

  await expect(page.locator('#tool-output')).toHaveText(REPLACE_OUTPUT, { timeout: 15000 });
});

test('target-mean-encoder deep-link pre-fills params and appends a smoothed column', async ({ page }) => {
  const params = new URLSearchParams({
    data: CSV,
    category: 'city',
    target: 'churn',
    smoothing: '3',
    leave_one_out: 'false',
    output: 'append',
    unknown: 'global-mean',
    decimals: '4',
    has_header: 'true',
    delimiter: 'comma',
  });

  await page.goto(`/tools/target-mean-encoder/?${params.toString()}`);
  await expect(page.locator('#in-category')).toHaveValue('city');
  await expect(page.locator('#in-output')).toHaveValue('append');
  await expect(page.locator('#tool-output')).toHaveText(APPEND_SMOOTHED_OUTPUT, { timeout: 15000 });
});

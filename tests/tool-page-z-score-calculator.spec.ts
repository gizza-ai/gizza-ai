import { test, expect } from './fixtures';

const scoreOutput = `Z-scores (1 value(s))

x = 130   z = 2   percentile = 97.724987%
  P(X < x) = 0.97725   P(X > x) = 0.02275   two-tailed p = 0.0455

Mean (μ) = 100   Standard deviation (σ) = 15`;

test('z-score-calculator page emits exact score output', async ({ page }) => {
  await page.goto('/tools/z-score-calculator/');
  await page.fill('#in-values', '130');
  await page.selectOption('#in-mode', 'score');
  await page.fill('#in-mean', '100');
  await page.fill('#in-std_dev', '15');
  await page.fill('#in-n', '1');
  await page.fill('#in-decimals', '6');

  await expect(page.locator('#tool-output')).toHaveText(scoreOutput, { timeout: 15_000 });
});

test('z-score-calculator honours deep-link critical mode', async ({ page }) => {
  const qs =
    '?values=' + encodeURIComponent('0.95, 0.975') +
    '&mode=critical' +
    '&mean=0' +
    '&std_dev=1' +
    '&n=1' +
    '&sample=false' +
    '&decimals=6';
  await page.goto('/tools/z-score-calculator/' + qs);

  await expect(page.locator('#in-mode')).toHaveValue('critical', { timeout: 15_000 });
  await expect(page.locator('#in-decimals')).toHaveValue('6');
  await expect(page.locator('#tool-output')).toHaveText(
    `Critical values (2 value(s))

x = 1.644854   z = 1.644854   percentile = 95%
  P(X < x) = 0.95   P(X > x) = 0.05   two-tailed p = 0.1
x = 1.959964   z = 1.959964   percentile = 97.5%
  P(X < x) = 0.975   P(X > x) = 0.025   two-tailed p = 0.05

Mean (μ) = 0   Standard deviation (σ) = 1`,
    { timeout: 15_000 },
  );
});

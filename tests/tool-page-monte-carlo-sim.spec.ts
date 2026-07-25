import { test, expect } from './fixtures';

const profitVars = 'revenue = normal(1000, 200)\ncost = uniform(400, 700)';

test('monte-carlo-sim runs a seeded simulation and reports the outcome distribution', async ({ page }) => {
  await page.goto('/tools/monte-carlo-sim/');
  await page.fill('#in-variables', profitVars);
  await page.fill('#in-model', 'revenue - cost');
  await page.fill('#in-trials', '20000');
  await page.fill('#in-seed', '42');
  await page.fill('#in-threshold', '0');
  await page.selectOption('#in-threshold_direction', 'at-or-above');
  await page.fill('#in-histogram_bins', '20');
  const out = page.locator('#tool-output');
  // Deterministic SplitMix64 PRNG → identical numbers across platforms.
  await expect(out).toContainText('Monte Carlo simulation — 20000 trials (seed 42)', { timeout: 15000 });
  await expect(out).toContainText('mean    : 447.211');
  await expect(out).toContainText('P50     : 445.2893  (median)');
  await expect(out).toContainText('P(outcome >= 0) = 98.095%');
  await expect(out).toContainText('Histogram:');
});

test('constant inputs collapse to an exact deterministic outcome', async ({ page }) => {
  await page.goto('/tools/monte-carlo-sim/');
  await page.fill('#in-variables', 'revenue = constant(1000)\ncost = constant(600)');
  await page.fill('#in-model', 'revenue - cost');
  await page.fill('#in-trials', '500');
  await page.fill('#in-seed', '1');
  await page.fill('#in-histogram_bins', '0');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('mean    : 400', { timeout: 15000 });
  await expect(out).toContainText('std dev : 0');
  await expect(out).toContainText('P99     : 400');
  // 0 bins hides the histogram entirely.
  await expect(out).not.toContainText('Histogram:');
});

test('at-or-below threshold flips the probability comparison', async ({ page }) => {
  await page.goto('/tools/monte-carlo-sim/');
  await page.fill('#in-variables', 'x = uniform(0, 100)');
  await page.fill('#in-model', 'x');
  await page.fill('#in-trials', '40000');
  await page.fill('#in-seed', '5');
  await page.fill('#in-threshold', '25');
  await page.selectOption('#in-threshold_direction', 'at-or-below');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('P(outcome <= 25) =', { timeout: 15000 });
});

test('an invalid distribution reports a clear error', async ({ page }) => {
  await page.goto('/tools/monte-carlo-sim/');
  await page.fill('#in-variables', 'x = weibull(1, 2)');
  await page.fill('#in-model', 'x');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('unknown distribution', { timeout: 15000 });
});

test('query-param deep-link prefills and runs', async ({ page }) => {
  await page.goto(
    '/tools/monte-carlo-sim/?variables=' +
      encodeURIComponent(profitVars) +
      '&model=' +
      encodeURIComponent('revenue - cost') +
      '&trials=20000&seed=42&threshold=0&threshold_direction=at-or-above&histogram_bins=20',
  );
  await expect(page.locator('#in-variables')).toHaveValue(profitVars, { timeout: 15000 });
  await expect(page.locator('#in-model')).toHaveValue('revenue - cost');
  await expect(page.locator('#in-threshold_direction')).toHaveValue('at-or-above');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Monte Carlo simulation — 20000 trials (seed 42)', { timeout: 15000 });
  await expect(out).toContainText('P(outcome >= 0) = 98.095%');
});

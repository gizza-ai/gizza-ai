import { test, expect } from './fixtures';

test('percentile-rank-calculator reports a default weak percentile rank with stats', async ({ page }) => {
  await page.goto('/tools/percentile-rank-calculator/');
  await page.waitForSelector('#in-data');
  await expect(page.locator('#in-method')).toHaveValue('weak');
  await expect(page.locator('#in-decimals')).toHaveValue('2');
  await expect(page.locator('#in-include_stats')).toBeChecked();

  await page.fill('#in-data', '6, 12, 13, 17, 17, 18, 20, 23, 24, 24, 25, 26, 27, 27, 30, 32, 33');
  await page.fill('#in-values', '25');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Percentile rank — weak method', { timeout: 20_000 });
  await expect(out).toContainText('25 → 64.71');
  await expect(out).toContainText('below: 10, equal: 1, above: 6');
  await expect(out).toContainText('Dataset summary');
});

test('percentile-rank-calculator deep link supports rank method and no summary', async ({ page }) => {
  await page.goto('/tools/percentile-rank-calculator/?data=1%2C%202%2C%202%2C%202%2C%205&values=2&method=rank&decimals=1&include_stats=false');
  await page.waitForSelector('#in-data');
  await expect(page.locator('#in-method')).toHaveValue('rank', { timeout: 15_000 });
  await expect(page.locator('#in-decimals')).toHaveValue('1');
  await expect(page.locator('#in-include_stats')).not.toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 → 60', { timeout: 20_000 });
  await expect(out).not.toContainText('Dataset summary');
});

test('percentile-rank-calculator page ships runnable CLI, labels, and preset chips', async ({ page }) => {
  await page.goto('/tools/percentile-rank-calculator/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(
    'gizza tool percentile-rank-calculator "6, 12, 13, 17, 17, 18, 20, 23, 24, 24, 25, 26, 27, 27, 30, 32, 33" \'values=25\''
  );
  await expect(page.locator('#in-method option[value="weak"]')).toHaveText('weak — count values ≤ target (common default)');
  await expect(page.locator('#in-method option[value="rank"]')).toHaveText('rank — average rank for tied values');
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
});

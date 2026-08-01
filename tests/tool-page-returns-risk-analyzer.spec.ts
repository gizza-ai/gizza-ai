import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('returns-risk-analyzer renders exact monthly risk metrics', async ({ page }) => {
  await page.goto('/tools/returns-risk-analyzer/');
  await page.fill('#in-returns', '1%, -0.5%, 2%');
  await page.selectOption('#in-periods_per_year', '12');
  await expect(page.locator('#tool-output')).toContainText('count: 3 returns', { timeout: 15000 });
  expect(await output(page)).toBe(
    'count: 3 returns\n' +
      'frequency: monthly (12 periods/year)\n' +
      'risk-free rate: 0.0000% / yr\n' +
      'Sortino target: 0.0000% / yr\n\n' +
      'period mean: 0.8333%\n' +
      'positive periods: 66.7%\n' +
      'best period: 2.0000%\n' +
      'worst period: -0.5000%\n' +
      'cumulative return: 2.5049%\n' +
      'annualized return: 10.4024%\n' +
      'annualized volatility: 4.3589%\n' +
      'downside deviation: 1.0000%\n' +
      'max drawdown: -0.5000%\n' +
      'Sharpe ratio: 2.2942\n' +
      'Sortino ratio: 10.0000\n' +
      'Calmar ratio: 20.8048\n\n' +
      'Volatility uses the sample standard deviation (÷ n−1); downside deviation divides by n. Annualized return is geometric (compound). Sharpe uses the risk-free rate; Sortino uses the target return as the minimum acceptable return. Educational only — not financial advice.',
  );
});

test('returns-risk-analyzer supports deep link with header and risk-free rate', async ({ page }) => {
  const returns = encodeURIComponent('return\n0.01\n0.03');
  await page.goto(`/tools/returns-risk-analyzer/?returns=${returns}&periods_per_year=12&risk_free_rate=12&target_return=0&has_header=true`);
  await expect(page.locator('#in-periods_per_year')).toHaveValue('12', { timeout: 15000 });
  await expect(page.locator('#in-has_header')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('risk-free rate: 12.0000% / yr');
  await expect(page.locator('#tool-output')).toContainText('count: 2 returns');
});

test('returns-risk-analyzer reports validation errors', async ({ page }) => {
  await page.goto('/tools/returns-risk-analyzer/');
  await page.fill('#in-returns', '0.01');
  await expect(page.locator('#tool-output')).toContainText('need at least 2 returns', { timeout: 15000 });
});

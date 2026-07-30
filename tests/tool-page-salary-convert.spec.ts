import { test, expect } from './fixtures';

test('salary-convert turns an hourly wage into every period figure', async ({ page }) => {
  await page.goto('/tools/salary-convert/');
  await page.fill('#in-amount', '25');
  await page.selectOption('#in-period', 'hourly');
  await page.fill('#in-hours_per_week', '40');
  await page.fill('#in-days_per_week', '5');
  await page.fill('#in-weeks_per_year', '52');

  const out = page.locator('#tool-output');
  // $25/hour × 40 × 52 = $52,000/year; monthly = annual ÷ 12; weekly = annual ÷ 52.
  await expect(out).toContainText('"annual": 52000.0', { timeout: 15000 });
  await expect(out).toContainText('"hourly": 25.0');
  await expect(out).toContainText('"weekly": 1000.0');
  await expect(out).toContainText('"biweekly": 2000.0');
  await expect(out).toContainText('"monthly": 4333.33');
  await expect(out).toContainText('"input_period": "hourly"');
});

test('salary-convert honours a part-time schedule and unpaid weeks', async ({ page }) => {
  await page.goto('/tools/salary-convert/');
  await page.fill('#in-amount', '30');
  await page.selectOption('#in-period', 'hourly');
  await page.fill('#in-hours_per_week', '20');
  await page.fill('#in-weeks_per_year', '50');

  const out = page.locator('#tool-output');
  // $30/hour × 20 h/week × 50 weeks = $30,000/year.
  await expect(out).toContainText('"annual": 30000.0', { timeout: 15000 });
  await expect(out).toContainText('"hours_per_week": 20.0');
  await expect(out).toContainText('"weeks_per_year": 50.0');
});

test('salary-convert deep-links a monthly salary via query params', async ({ page }) => {
  await page.goto('/tools/salary-convert/?amount=5000&period=monthly&hours_per_week=40&days_per_week=5&weeks_per_year=52&currency=%C2%A3');

  const out = page.locator('#tool-output');
  // $5,000/month × 12 = $60,000/year; the £ symbol shows only in the summary text.
  await expect(out).toContainText('"annual": 60000.0', { timeout: 15000 });
  await expect(out).toContainText('"input_period": "monthly"');
  await expect(out).toContainText('£60,000.00');
});

import { test, expect } from './fixtures';

test('date-diff page computes calendar breakdown', async ({ page }) => {
  await page.goto('/tools/date-diff/');

  await page.fill('#in-start', '2020-01-15');
  await page.fill('#in-end', '2022-03-20');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"years": 2', { timeout: 15000 });
  await expect(out).toContainText('"months": 2');
  await expect(out).toContainText('"days": 5');
  await expect(out).toContainText('2 years, 2 months and 5 days');
  await expect(out).toContainText('"total_days": 795');
});

test('date-diff page handles datetime + flat totals', async ({ page }) => {
  await page.goto('/tools/date-diff/');

  await page.fill('#in-start', '2024-01-01T08:00:00');
  await page.fill('#in-end', '2024-01-02T10:30:15');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"hours": 2', { timeout: 15000 });
  await expect(out).toContainText('"minutes": 30');
  await expect(out).toContainText('"seconds": 15');
  await expect(out).toContainText('"total_hours": 26');
});

test('date-diff page flags reversed order', async ({ page }) => {
  await page.goto('/tools/date-diff/');

  await page.fill('#in-start', '2024-06-01');
  await page.fill('#in-end', '2024-01-01');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"negative": true', { timeout: 15000 });
  await expect(out).toContainText('end is before start');
});

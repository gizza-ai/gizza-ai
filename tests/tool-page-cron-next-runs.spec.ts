import { test, expect } from './fixtures';

test('cron-next-runs page: next run times for */15 (text)', async ({ page }) => {
  await page.goto('/tools/cron-next-runs/');

  // Pin the base time so the output is deterministic regardless of run date.
  await page.fill('#in-after', '2020-01-01T00:00:00Z');
  await page.fill('#in-count', '3');
  await page.fill('#in-expression', '*/15 * * * *');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Expression:  */15 * * * *', { timeout: 15000 });
  await expect(out).toContainText('Schedule:    Every 15 minutes (UTC)');
  await expect(out).toContainText('After (UTC): Wednesday 2020-01-01T00:00:00Z');
  await expect(out).toContainText('1. Wednesday 2020-01-01T00:15:00Z');
  await expect(out).toContainText('2. Wednesday 2020-01-01T00:30:00Z');
  await expect(out).toContainText('3. Wednesday 2020-01-01T00:45:00Z');
});

test('cron-next-runs page: weekday names + JSON output', async ({ page }) => {
  await page.goto('/tools/cron-next-runs/');

  await page.fill('#in-after', '2020-01-01T00:00:00Z');
  await page.fill('#in-count', '2');
  await page.selectOption('#in-format', 'json');
  await page.fill('#in-expression', '0 9 * * MON-FRI');

  const out = page.locator('#tool-output');
  // 2020-01-01 is a Wednesday, so the first weekday 09:00 is that same day.
  await expect(out).toContainText('"description": "At 09:00, Monday through Friday (UTC)"', {
    timeout: 15000,
  });
  await expect(out).toContainText('"iso": "2020-01-01T09:00:00Z"');
  await expect(out).toContainText('"weekday": "Wednesday"');
  await expect(out).toContainText('"iso": "2020-01-02T09:00:00Z"');
  await expect(out).toContainText('"epoch": 1577869200');
});

test('cron-next-runs page: invalid expression shows an error', async ({ page }) => {
  await page.goto('/tools/cron-next-runs/');

  await page.fill('#in-after', '2020-01-01T00:00:00Z');
  await page.fill('#in-expression', '99 * * * *');

  // The page surfaces the core error message in the output element.
  await expect(page.locator('#tool-output')).toContainText('out of range', { timeout: 15000 });
});

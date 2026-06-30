import { test, expect } from './fixtures';

test('time-to-decimal-hours page converts clock duration to decimal hours', async ({ page }) => {
  await page.goto('/tools/time-to-decimal-hours/');
  await page.fill('#in-value', '1:30');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"detected_input": "clock"', { timeout: 15000 });
  await expect(out).toContainText('"clock": "1:30"');
  await expect(out).toContainText('"decimal_hours": 1.5');
  await expect(out).toContainText('"total_minutes": 90');
});

test('time-to-decimal-hours page converts decimal hours back to clock', async ({ page }) => {
  await page.goto('/tools/time-to-decimal-hours/');
  await page.selectOption('#in-mode', 'from-decimal');
  await page.fill('#in-value', '2.25');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"detected_input": "decimal"', { timeout: 15000 });
  await expect(out).toContainText('"clock": "2:15"');
  await expect(out).toContainText('"total_seconds": 8100');
});

test('time-to-decimal-hours query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/time-to-decimal-hours/?value=' + encodeURIComponent('0:20') + '&mode=from-clock',
  );
  await expect(page.locator('#in-value')).toHaveValue('0:20', { timeout: 15000 });
  await expect(page.locator('#in-mode')).toHaveValue('from-clock');
  await expect(page.locator('#tool-output')).toContainText('"decimal_hours": 0.3333', {
    timeout: 15000,
  });
});

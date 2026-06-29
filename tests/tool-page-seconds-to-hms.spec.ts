import { test, expect } from './fixtures';

test('seconds-to-hms page converts seconds to default hms', async ({ page }) => {
  await page.goto('/tools/seconds-to-hms/');
  await page.fill('#in-seconds', '5025');
  await expect(page.locator('#tool-output')).toContainText('01:23:45', { timeout: 15000 });
});

test('seconds-to-hms page supports days, iso, words, and fractional seconds', async ({ page }) => {
  await page.goto('/tools/seconds-to-hms/');
  await page.fill('#in-seconds', '90061');
  await page.fill('#in-format', 'dhms');
  await expect(page.locator('#tool-output')).toContainText('1:01:01:01', { timeout: 15000 });

  await page.fill('#in-format', 'iso');
  await expect(page.locator('#tool-output')).toContainText('P1DT1H1M1S');

  await page.fill('#in-format', 'words');
  await expect(page.locator('#tool-output')).toContainText('1 day, 1 hour, 1 minute, 1 second');

  await page.fill('#in-seconds', '90.5');
  await page.fill('#in-format', 'hms');
  await page.fill('#in-decimals', '1');
  await expect(page.locator('#tool-output')).toContainText('00:01:30.5');
});

test('seconds-to-hms page supports query-param deep links and validation errors', async ({ page }) => {
  await page.goto('/tools/seconds-to-hms/?seconds=61&format=auto&decimals=0');
  await expect(page.locator('#in-seconds')).toHaveValue('61');
  await expect(page.locator('#tool-output')).toContainText('01:01', { timeout: 15000 });

  await page.fill('#in-format', 'bogus');
  await expect(page.locator('#tool-output')).toContainText('invalid format');
});

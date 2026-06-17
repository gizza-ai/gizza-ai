import { test, expect } from './fixtures';

test('phone-format page — valid international number', async ({ page }) => {
  await page.goto('/tools/phone-format/');
  await page.fill('#in-number', '+14155552671');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid: yes', { timeout: 15000 });
  await expect(out).toContainText('E.164: +14155552671');
  await expect(out).toContainText('Country/region: US');
});

test('phone-format page — national number with region', async ({ page }) => {
  await page.goto('/tools/phone-format/');
  await page.fill('#in-number', '4155552671');
  await page.fill('#in-region', 'US');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('E.164: +14155552671', { timeout: 15000 });
});

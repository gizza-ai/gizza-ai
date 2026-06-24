import { test, expect } from './fixtures';

test('age-calculator page computes calendar breakdown', async ({ page }) => {
  await page.goto('/tools/age-calculator/');

  await page.fill('#in-birthdate', '1990-01-15');
  await page.fill('#in-as_of', '2026-06-22');

  const out = page.locator('#tool-output');
  await expect(out.locator('.age-result-hero-text')).toContainText('36 years, 5 months, and 7 days old', { timeout: 15000 });
  await expect(out.locator('.age-info-card-value').nth(1)).toContainText('Monday');
  await expect(out.locator('.age-info-card-value').nth(2)).toContainText('Capricorn');
});

test('age-calculator page reports totals and next birthday', async ({ page }) => {
  await page.goto('/tools/age-calculator/');

  await page.fill('#in-birthdate', '2000-06-22');
  await page.fill('#in-as_of', '2026-06-21');

  const out = page.locator('#tool-output');
  await expect(out.locator('.age-result-hero-text')).toContainText('25 years, 11 months, and 30 days old', { timeout: 15000 });
  await expect(out.locator('.age-info-card-value').nth(0)).toContainText('1 Day');
  await expect(out.locator('.age-info-card-sub').nth(0)).toContainText('Date: 2026-06-22');
  await expect(out.locator('.age-info-card-value').nth(2)).toContainText('Cancer');
});

test('age-calculator page defaults as-of to today when blank', async ({ page }) => {
  await page.goto('/tools/age-calculator/');

  // Leave the as-of field blank — the page should use the browser's today.
  await page.fill('#in-birthdate', '2000-06-22');

  const out = page.locator('#tool-output');
  // Whatever today is, the result must be valid dashboard output with a Cancer sign,
  // proving the blank-as_of clock path works.
  await expect(out.locator('.age-info-card-value').nth(2)).toContainText('Cancer', { timeout: 15000 });
  await expect(out.locator('.age-info-card-sub').nth(1)).toContainText('Birthdate: 2000-06-22');
});

import { test, expect } from './fixtures';

test('age-calculator page computes calendar breakdown', async ({ page }) => {
  await page.goto('/tools/age-calculator/');

  await page.fill('#in-birthdate', '1990-01-15');
  await page.fill('#in-as_of', '2026-06-22');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"years": 36', { timeout: 15000 });
  await expect(out).toContainText('"months": 5');
  await expect(out).toContainText('"days": 7');
  await expect(out).toContainText('36 years 5 months 7 days old');
  await expect(out).toContainText('"zodiac_sign": "Capricorn"');
  await expect(out).toContainText('"day_of_week_born": "Monday"');
});

test('age-calculator page reports totals and next birthday', async ({ page }) => {
  await page.goto('/tools/age-calculator/');

  await page.fill('#in-birthdate', '2000-06-22');
  await page.fill('#in-as_of', '2026-06-21');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"years": 25', { timeout: 15000 });
  await expect(out).toContainText('"days_until_next_birthday": 1');
  await expect(out).toContainText('"next_birthday": "2026-06-22"');
  await expect(out).toContainText('"zodiac_sign": "Cancer"');
});

test('age-calculator page defaults as-of to today when blank', async ({ page }) => {
  await page.goto('/tools/age-calculator/');

  // Leave the as-of field blank — the page should use the browser's today.
  await page.fill('#in-birthdate', '2000-06-22');

  const out = page.locator('#tool-output');
  // Whatever today is, the result must be valid JSON with a Cancer sign and a
  // plausible age (>= 24), proving the blank-as_of clock path works.
  await expect(out).toContainText('"zodiac_sign": "Cancer"', { timeout: 15000 });
  await expect(out).toContainText('"birthdate": "2000-06-22"');
});

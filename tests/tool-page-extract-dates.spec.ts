import { test, expect } from './fixtures';

// /tools/extract-dates/ finds + normalizes dates in-browser (pure wasm).
test('extract-dates normalizes mixed date formats to ISO 8601', async ({ page }) => {
  await page.goto('/tools/extract-dates/');
  await page.fill(
    '#in-text',
    'We met on January 5, 2024 at 3pm, then again 2024-02-10 14:30 and 25/12/2024.',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('4 found', { timeout: 15000 });
  await expect(out).toContainText('2024-01-05');
  await expect(out).toContainText('15:00:00');
  await expect(out).toContainText('2024-02-10T14:30:00');
  await expect(out).toContainText('2024-12-25');
});

test('extract-dates reports when none are present', async ({ page }) => {
  await page.goto('/tools/extract-dates/');
  await page.fill('#in-text', 'there is nothing date-like in this sentence');
  await expect(page.locator('#tool-output')).toContainText('No dates', { timeout: 15000 });
});

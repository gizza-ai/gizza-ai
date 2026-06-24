import { test, expect } from './fixtures';

// /tools/parse-datetime/ parses a date/time string into structured parts (pure wasm).
test('parse-datetime parses an ISO datetime with offset', async ({ page }) => {
  await page.goto('/tools/parse-datetime/');
  await page.fill('#in-input', '2024-01-05T14:30:00+02:00');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('datetime', { timeout: 15000 });
  await expect(out).toContainText('2024-01-05T14:30:00+02:00');
  await expect(out).toContainText('2024'); // year
  await expect(out).toContainText('January'); // month name
  await expect(out).toContainText('Friday'); // weekday
  await expect(out).toContainText('+02:00'); // UTC offset
});

test('parse-datetime parses a US slash date', async ({ page }) => {
  await page.goto('/tools/parse-datetime/');
  await page.fill('#in-input', '01/05/2024');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('date', { timeout: 15000 });
  await expect(out).toContainText('January');
  await expect(out).toContainText('2024-01-05');
});

test('parse-datetime parses a bare clock time', async ({ page }) => {
  await page.goto('/tools/parse-datetime/');
  await page.fill('#in-input', '3pm');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('time', { timeout: 15000 });
  await expect(out).toContainText('15:00:00');
});

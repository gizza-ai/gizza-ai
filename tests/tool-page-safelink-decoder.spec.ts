import { test, expect } from './fixtures';
test('safelink-decoder page unwraps a SafeLink', async ({ page }) => {
  await page.goto('/tools/safelink-decoder/');
  await page.fill('#in-url', 'https://nam.safelinks.protection.outlook.com/?url=https%3A%2F%2Fexample.com%2Fp&data=x');
  await expect(page.locator('#tool-output')).toHaveText('https://example.com/p', { timeout: 15000 });
});
test('safelink-decoder query-param deep-link', async ({ page }) => {
  const wrapped = 'https://www.google.com/url?q=https%3A%2F%2Fexample.org%2Fz&sa=D';
  await page.goto('/tools/safelink-decoder/?url=' + encodeURIComponent(wrapped));
  await expect(page.locator('#in-url')).toHaveValue(wrapped, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('https://example.org/z', { timeout: 15000 });
});

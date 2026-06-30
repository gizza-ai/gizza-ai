import { test, expect } from './fixtures';

// /tools/luhn-checkdigit/ computes the Luhn check digit in-browser (pure wasm).
test('luhn-checkdigit page computes the check digit for a Visa test card prefix', async ({ page }) => {
  await page.goto('/tools/luhn-checkdigit/');
  await page.fill('#in-number', '424242424242424');
  await expect(page.locator('#tool-output')).toContainText('Check digit: 2', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('4242424242424242');
});

test('luhn-checkdigit page completes a number via deep-link', async ({ page }) => {
  await page.goto('/tools/luhn-checkdigit/?number=' + encodeURIComponent('7992739871'));
  await expect(page.locator('#tool-output')).toContainText('Check digit: 3', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('79927398713');
});

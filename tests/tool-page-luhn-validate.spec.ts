import { test, expect } from './fixtures';

// /tools/luhn-validate/ runs the Luhn check in-browser (pure wasm).
test('luhn-validate page validates a Visa test card', async ({ page }) => {
  await page.goto('/tools/luhn-validate/');
  await page.fill('#in-number', '4242 4242 4242 4242');
  await expect(page.locator('#tool-output')).toContainText('VALID', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Visa');
});

test('luhn-validate page flags an invalid number via deep-link', async ({ page }) => {
  await page.goto('/tools/luhn-validate/?number=' + encodeURIComponent('4242424242424241'));
  await expect(page.locator('#tool-output')).toContainText('INVALID', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('would be 2');
});

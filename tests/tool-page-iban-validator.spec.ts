import { test, expect } from './fixtures';

// /tools/iban-validator/ validates an IBAN in-browser (pure wasm, mod-97).
test('iban-validator page validates a UK IBAN', async ({ page }) => {
  await page.goto('/tools/iban-validator/');
  await page.fill('#in-iban', 'GB82 WEST 1234 5698 7654 32');
  await expect(page.locator('#tool-output')).toContainText('VALID', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('United Kingdom');
  await expect(page.locator('#tool-output')).toContainText('Bank code: WEST');
});

test('iban-validator page flags a bad checksum via deep-link', async ({ page }) => {
  await page.goto('/tools/iban-validator/?iban=' + encodeURIComponent('GB83WEST12345698765432'));
  await expect(page.locator('#tool-output')).toContainText('INVALID', { timeout: 15000 });
});

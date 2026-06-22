import { test, expect } from './fixtures';

test('bip39-mnemonic-generator page', async ({ page }) => {
  await page.goto('/tools/bip39-mnemonic-generator/');

  // Deterministic path: supply the all-zero 128-bit entropy + TREZOR passphrase
  // → the canonical BIP39 test vector.
  await page.fill('#in-entropy_hex', '00000000000000000000000000000000');
  await page.fill('#in-passphrase', 'TREZOR');
  await expect(page.locator('#tool-output')).toContainText(
    'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about',
    { timeout: 15000 },
  );
  await expect(page.locator('#tool-output')).toContainText(
    'c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04',
    { timeout: 15000 },
  );

  // Random path: blank entropy + 256-bit strength → a 24-word mnemonic.
  await page.fill('#in-entropy_hex', '');
  await page.selectOption('#in-strength', '256');
  await expect(page.locator('#tool-output')).toContainText('Mnemonic (24 words):', {
    timeout: 15000,
  });
});

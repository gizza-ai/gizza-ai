import { test, expect } from './fixtures';

const MNEMONIC = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
const TREZOR_SEED = 'c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('bip39-seed-derive page derives canonical TREZOR vector', async ({ page }) => {
  await page.goto('/tools/bip39-seed-derive/');
  await page.fill('#in-mnemonic', MNEMONIC);
  await page.fill('#in-passphrase', 'TREZOR');
  await expect(page.locator('#tool-output')).toContainText(TREZOR_SEED, { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('BIP39 seed (512-bit, hex):');
  expect(text).toContain('Mnemonic (12 words, valid checksum):');
  expect(text).toContain('Recovered entropy (128 bits): 00000000000000000000000000000000');
  expect(text).toContain('Passphrase: TREZOR');
});

test('bip39-seed-derive page rejects a bad checksum', async ({ page }) => {
  await page.goto('/tools/bip39-seed-derive/');
  await page.fill('#in-mnemonic', 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon');
  await expect(page.locator('#tool-output')).toContainText('invalid BIP39 checksum', { timeout: 15000 });
});

test('bip39-seed-derive deep-link pre-fills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/bip39-seed-derive/?mnemonic=' +
      encodeURIComponent(MNEMONIC) +
      '&passphrase=' +
      encodeURIComponent('TREZOR'),
  );
  await expect(page.locator('#in-mnemonic')).toHaveValue(MNEMONIC, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText(TREZOR_SEED, { timeout: 15000 });
  expect(await outputText(page)).toContain('Passphrase: TREZOR');
});

import { test, expect } from './fixtures';

const K1 = '0000000000000000000000000000000000000000000000000000000000000001';

async function setKey(page: any, value: string) {
  await page.fill('#in-key', value);
}

test('bitcoin-address page derives mainnet compressed known vector', async ({ page }) => {
  await page.goto('/tools/bitcoin-address/');
  await setKey(page, K1);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('network: mainnet', { timeout: 15000 });
  await expect(out).toContainText('compressed: true');
  await expect(out).toContainText('p2pkh: 1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH');
  await expect(out).toContainText('private_key_wif: KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn');
  await expect(out).toContainText('p2wpkh: bc1q');
});

test('bitcoin-address page supports uncompressed non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/bitcoin-address/');
  await page.uncheck('#in-compressed');
  await setKey(page, K1);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('compressed: false', { timeout: 15000 });
  await expect(out).toContainText('p2pkh: 1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm');
  await expect(out).toContainText('private_key_wif: 5HpHagT65TZzG1PH3CSu63k8DbpvD8s5ip4nEB3kEsreAnchuDf');
  await expect(out).toContainText('p2wpkh: (requires a compressed key');
});

test('bitcoin-address query params prefill testnet and compute', async ({ page }) => {
  await page.goto('/tools/bitcoin-address/?key=' + K1 + '&network=testnet&compressed=true');

  await expect(page.locator('#in-key')).toHaveValue(K1, { timeout: 15000 });
  await expect(page.locator('#in-network')).toHaveValue('testnet');
  await expect(page.locator('#in-compressed')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('network: testnet', { timeout: 15000 });
  await expect(out).toContainText('p2wpkh: tb1q');
  await expect(out).toContainText('private_key_wif: c');
});

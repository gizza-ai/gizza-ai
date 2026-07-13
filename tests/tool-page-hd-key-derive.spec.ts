import { test, expect } from './fixtures';

const SEED = '000102030405060708090a0b0c0d0e0f';

test('hd-key-derive derives BIP32 vector child key on the page', async ({ page }) => {
  await page.goto('/tools/hd-key-derive/');
  await page.fill('#in-seed', SEED);
  await page.fill('#in-path', "m/0h");
  await page.selectOption('#in-network', 'mainnet');
  await page.selectOption('#in-address_type', 'p2pkh');

  const out = page.locator('#tool-output');
  await expect(out).toContainText("path: m/0'", { timeout: 15000 });
  await expect(out).toContainText('xprv: xprv9uHRZZhk6KAJC1avXpDAp4MDc3sQKNxDiPvvkX8Br5ngLNv1TxvUxt4cV1rGL5hj6KCesnDYUhd7oWgT11eZG7XnxHrnYeSvkzY7d2bhkJ7');
  await expect(out).toContainText('xpub: xpub68Gmy5EdvgibQVfPdqkBBCHxA5htiqg55crXYuXoQRKfDBFA1WEjWgP6LHhwBZeNK1VTsfTFUHCdrfp1bgwQ9xv5ski8PX9rL2dZXvgGDnw');
  await expect(out).toContainText('private_key_wif: L5BmPijJjrKbiUfG4zbiFKNqkvuJ8usooJmzuD7Z8dkRoTThYnAT');
  await expect(out).toContainText('address: 19Q2WoS5hSS6T8GjhK8KZLMgmWaq4neXrh');
});

test('hd-key-derive supports native segwit testnet deep link', async ({ page }) => {
  const params = new URLSearchParams({
    seed: SEED,
    path: "m/84'/1'/0'/0/0",
    network: 'testnet',
    address_type: 'p2wpkh',
  });
  await page.goto(`/tools/hd-key-derive/?${params.toString()}`);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('network: testnet', { timeout: 15000 });
  const text = (await out.textContent()) || '';
  expect(text).toContain('address_type: p2wpkh');
  expect(text).toContain('xprv: tprv8k7UWWmyp8CB2hAQsgWiSEWJtGPHHaF6Jb4e9MaqtTtKG6e2VmZot2TnpFRXjrZkTMcRpRcdh39kd4F9ZB8WN8mLH9VCpA6BMjM1cVxoZ68');
  expect(text).toContain('address: tb1q7f0pjwhc3jzzv0w4uurm589506glv2dg2qy7ze');
});

test('hd-key-derive renders wrapped-segwit address type', async ({ page }) => {
  await page.goto('/tools/hd-key-derive/');
  await page.fill('#in-seed', SEED);
  await page.fill('#in-path', "m/0'");
  await page.selectOption('#in-address_type', 'p2sh_p2wpkh');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('address_type: p2sh_p2wpkh', { timeout: 15000 });
  await expect(out).toContainText('address: 3AbBmNbPDSzeZKHywDrH3h5v2rL8xGfT7e');
});

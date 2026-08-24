import { test, expect } from './fixtures';

const PRIV_ONE = '0000000000000000000000000000000000000000000000000000000000000001';
const G_COMPRESSED = '0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798';
const G_RAW_XY = '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8';
const CHECKSUM = '0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf';
const LOWER = '0x7e5f4552091a69125d5dfcb7b8c2659029395bdf';

test('eth-address-from-key page derives EIP-55 checksum from a private key', async ({ page }) => {
  await page.goto('/tools/eth-address-from-key/');
  await page.fill('#in-key', PRIV_ONE);
  await page.selectOption('#in-key_type', 'private');
  await page.selectOption('#in-output_format', 'checksum');

  await expect(page.locator('#tool-output')).toHaveText(CHECKSUM, { timeout: 15_000 });
});

test('eth-address-from-key page accepts compressed public keys and lowercase output', async ({ page }) => {
  await page.goto('/tools/eth-address-from-key/');
  await page.fill('#in-key', G_COMPRESSED);
  await page.selectOption('#in-key_type', 'public');
  await page.selectOption('#in-output_format', 'lowercase');

  await expect(page.locator('#tool-output')).toHaveText(LOWER, { timeout: 15_000 });
});

test('eth-address-from-key deep link prefills params and returns JSON', async ({ page }) => {
  const params = new URLSearchParams({
    key: G_RAW_XY,
    key_type: 'public',
    output_format: 'json',
  });
  await page.goto(`/tools/eth-address-from-key/?${params.toString()}`);

  await expect(page.locator('#in-key')).toHaveValue(G_RAW_XY, { timeout: 15_000 });
  await expect(page.locator('#in-key_type')).toHaveValue('public');
  await expect(page.locator('#in-output_format')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText(`"address": "${CHECKSUM}"`, { timeout: 15_000 });
  await expect(out).toContainText(`"lowercase": "${LOWER}"`);
  await expect(out).toContainText(`"public_key_compressed": "${G_COMPRESSED}"`);
});

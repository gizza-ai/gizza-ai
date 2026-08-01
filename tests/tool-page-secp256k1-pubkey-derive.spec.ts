import { test, expect } from './fixtures';

const KEY_ONE = '0000000000000000000000000000000000000000000000000000000000000001';
const COMPRESSED = '0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798';
const X_ONLY = '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798';

test('secp256k1-pubkey-derive page derives generator point compressed key', async ({ page }) => {
  await page.goto('/tools/secp256k1-pubkey-derive/');
  await page.fill('#in-key', KEY_ONE);
  await page.selectOption('#in-format', 'compressed');
  await expect(page.locator('#tool-output')).toHaveText(COMPRESSED, { timeout: 15_000 });
});

test('secp256k1-pubkey-derive deep link returns x-only coordinate', async ({ page }) => {
  const params = new URLSearchParams({ key: KEY_ONE, format: 'x' });
  await page.goto(`/tools/secp256k1-pubkey-derive/?${params.toString()}`);
  await expect(page.locator('#in-key')).toHaveValue(KEY_ONE, { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('x');
  await expect(page.locator('#tool-output')).toHaveText(X_ONLY, { timeout: 15_000 });
});

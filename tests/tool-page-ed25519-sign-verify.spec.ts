import { test, expect } from './fixtures';

const SEED = '4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb';
const PUB = '3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c';
const SIG = '92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

test('ed25519-sign-verify page signs RFC 8032 vector exactly', async ({ page }) => {
  await page.goto('/tools/ed25519-sign-verify/');
  await page.selectOption('#in-operation', 'sign');
  await page.fill('#in-message', '72');
  await page.selectOption('#in-message_encoding', 'hex');
  await page.fill('#in-key', SEED);
  await expect(page.locator('#tool-output')).toContainText(`signature (hex): ${SIG}`, { timeout: 15000 });
  const text = await outText(page);
  expect(text).toContain('operation: sign');
  expect(text).toContain('length: 64 bytes');
  expect(text).toContain(`public key (hex): ${PUB}`);
});

test('ed25519-sign-verify page verifies a valid signature', async ({ page }) => {
  await page.goto('/tools/ed25519-sign-verify/');
  await page.selectOption('#in-operation', 'verify');
  await page.fill('#in-message', '72');
  await page.selectOption('#in-message_encoding', 'hex');
  await page.fill('#in-key', PUB);
  await page.fill('#in-signature', SIG);
  await expect(page.locator('#tool-output')).toContainText('valid: true', { timeout: 15000 });
  expect(await outText(page)).toContain('✓ signature is valid');
});

test('ed25519-sign-verify page reports wrong message as valid false', async ({ page }) => {
  await page.goto('/tools/ed25519-sign-verify/');
  await page.selectOption('#in-operation', 'verify');
  await page.fill('#in-message', '73');
  await page.selectOption('#in-message_encoding', 'hex');
  await page.fill('#in-key', PUB);
  await page.fill('#in-signature', SIG);
  await expect(page.locator('#tool-output')).toContainText('valid: false', { timeout: 15000 });
});

test('ed25519-sign-verify deep-link signs base64 message', async ({ page }) => {
  await page.goto(`/tools/ed25519-sign-verify/?operation=sign&message=aGk%3D&message_encoding=base64&key=${SEED}`);
  await expect(page.locator('#tool-output')).toContainText('operation: sign', { timeout: 15000 });
  expect(await outText(page)).toContain('signature (hex):');
});

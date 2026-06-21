import { test, expect } from './fixtures';

const P256_KEY =
  '-----BEGIN PRIVATE KEY-----\n' +
  'MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgn5zu0/Tig4Q969rM\n' +
  'ujaKR8Y42hSK02jb3UXaRk2OKNOhRANCAAQdDtizrLTz7++OkxNxnFU2fCcErxbN\n' +
  'Gd7Vego6C920F1p4GGHX/1dxpgOU3PTUC3A/mlUN7rXTzzWCRHtm95Nm\n' +
  '-----END PRIVATE KEY-----\n';

// /tools/ecdsa-sign/ signs a message with an EC key in-browser (pure wasm).
test('ecdsa-sign produces a deterministic P-256 DER signature', async ({ page }) => {
  await page.goto('/tools/ecdsa-sign/');
  await page.fill('#in-message', 'hello world');
  await page.fill('#in-private_key', P256_KEY);
  await page.selectOption('#in-curve', 'p256');
  await page.selectOption('#in-format', 'der');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('curve: p256', { timeout: 15000 });
  await expect(out).toContainText('format: der');
  // RFC-6979 deterministic signature for this key+message (base64, DER).
  await expect(out).toContainText('signature (base64):');
  await expect(out).toContainText('signature (hex): 3046022100fa23f6c244eabb59');
});

test('ecdsa-sign raw P-256 signature is 64 bytes', async ({ page }) => {
  await page.goto('/tools/ecdsa-sign/');
  await page.fill('#in-message', 'msg');
  await page.fill('#in-private_key', P256_KEY);
  await page.selectOption('#in-curve', 'p256');
  await page.selectOption('#in-format', 'raw');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('length: 64 bytes', { timeout: 15000 });
});

import { test, expect } from './fixtures';
import * as fs from 'fs';
import * as path from 'path';

const FIX = path.join(__dirname, 'fixtures');
const SYMMETRIC = fs.readFileSync(path.join(FIX, 'pgp-decrypt-symmetric.asc'), 'utf-8');

test('pgp-decrypt page decrypts a password-encrypted message', async ({ page }) => {
  await page.goto('/tools/pgp-decrypt/');
  await page.fill('#in-message', SYMMETRIC);
  await page.fill('#in-passphrase', 'open sesame');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"plaintext": "cli decrypt ok"', { timeout: 15000 });
  await expect(out).toContainText('"encryption": "password"');
  await expect(out).toContainText('"output_format": "text"');
});

test('pgp-decrypt page deep-link prefill can request hex output', async ({ page }) => {
  const params = new URLSearchParams({
    message: SYMMETRIC,
    passphrase: 'open sesame',
    output_format: 'hex',
  });
  await page.goto(`/tools/pgp-decrypt/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"plaintext": "636c692064656372797074206f6b"', { timeout: 15000 });
  await expect(out).toContainText('"output_format": "hex"');
});

test('pgp-decrypt page reports a wrong password clearly', async ({ page }) => {
  await page.goto('/tools/pgp-decrypt/');
  await page.fill('#in-message', SYMMETRIC);
  await page.fill('#in-passphrase', 'wrong password');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('decryption failed with that password', { timeout: 15000 });
});

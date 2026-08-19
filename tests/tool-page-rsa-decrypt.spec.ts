import { readFileSync } from 'fs';
import { test, expect } from './fixtures';

const tool = '/tools/rsa-decrypt/';
const privateKey = readFileSync('../blocks/rsa-decrypt/core/tests/test-key.pem', 'utf8');
const ciphertext =
  'P3ogbhFSdYhbKDxOIgZBdxwBOtTLVkq0gV4yNLjZVwh4aFoeP+sEy6gy0zmAu4g5xLwZpvu64ZU8PCUJhoNnF3H2fTy2e3pHjETOzAx1AgxscjzXimsPqRk0wn01t653brCIe5HM6zCesJTyKN3HpnVTdWVH/z5YVsekiuxqNXxxYDhVFJZASJq84/FP5qaOaoN+cHPSzqEhu1hkez8CtdXGYxf+uCuiMAFcCBoF8CiPK33Bwu7SanbM0BgGMHVtYsO25D00Sj3M+TLFrq5pnvgz6iQT0K9BJfuVu1y/vOX29ypTO2elyp+LrZZ5EAt5MSde+7bQ5s1u/5TcAWYRig==';

async function runWasm(
  page,
  ct: string,
  key: string,
  passphrase = '',
  padding = 'oaep',
  hash = 'sha256',
  ciphertextEncoding = 'base64',
  outputEncoding = 'utf8',
): Promise<string> {
  return await page.evaluate(
    async ({ ct, key, passphrase, padding, hash, ciphertextEncoding, outputEncoding }) => {
      const mod = await import('/tools/rsa-decrypt/gizza_ai_rsa_decrypt_web.js');
      await mod.default('/tools/rsa-decrypt/gizza_ai_rsa_decrypt_web_bg.wasm');
      return mod.run(ct, key, passphrase, padding, hash, ciphertextEncoding, outputEncoding);
    },
    { ct, key, passphrase, padding, hash, ciphertextEncoding, outputEncoding },
  );
}

test('rsa-decrypt page bundle decrypts OAEP SHA-256 ciphertext', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-ciphertext');
  await expect(runWasm(page, ciphertext, privateKey)).resolves.toBe('hello from rsa-decrypt');
});

test('rsa-decrypt deep link pre-fills non-secret parameters and runs after key paste', async ({ page }) => {
  const qs = new URLSearchParams({
    ciphertext,
    padding: 'oaep',
    hash: 'sha256',
    ciphertext_encoding: 'base64',
    output_encoding: 'utf8',
  });
  await page.goto(`${tool}?${qs.toString()}`);
  await expect(page.locator('#in-ciphertext')).toHaveValue(ciphertext, { timeout: 15_000 });
  await expect(page.locator('#in-padding')).toHaveValue('oaep');
  await expect(page.locator('#in-hash')).toHaveValue('sha256');
  await expect(runWasm(page, ciphertext, privateKey)).resolves.toBe('hello from rsa-decrypt');
});

test('rsa-decrypt wasm covers encodings, padding choices, and validation', async ({ page }) => {
  await page.goto(tool);
  await page.waitForSelector('#in-ciphertext');

  await expect(runWasm(page, ciphertext, privateKey)).resolves.toBe('hello from rsa-decrypt');
  await expect(runWasm(page, ciphertext, privateKey, '', 'oaep', 'sha256', 'base64', 'hex')).resolves.toBe(
    '68656c6c6f2066726f6d207273612d64656372797074',
  );
  await expect(runWasm(page, ciphertext, privateKey, '', 'oaep', 'sha256', 'base64', 'base64')).resolves.toBe(
    'aGVsbG8gZnJvbSByc2EtZGVjcnlwdA==',
  );
  await expect(runWasm(page, ciphertext, privateKey, '', 'oaep', 'sha512')).rejects.toThrow(/decryption failed/);
  await expect(runWasm(page, 'AAAA', privateKey)).rejects.toThrow(/expects exactly 256 bytes/);
  await expect(runWasm(page, ciphertext, 'not a key')).rejects.toThrow(/no RSA private key found/);
});

test('rsa-decrypt ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'OAEP SHA-256 text',
    'Hex ciphertext → hex plaintext',
    'Legacy PKCS#1 v1.5',
  ]);
});

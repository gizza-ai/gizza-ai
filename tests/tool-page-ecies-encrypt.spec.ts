import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';

const RECIPIENT_SK = '1111111111111111111111111111111111111111111111111111111111111111';
const RECIPIENT_PUB =
  '044f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa385b6b1b8ead809ca67454d9683fcf2ba03456d6fe2c4abe2b07f0fbdbb2f1c1';
const EPHEMERAL_SK = '2222222222222222222222222222222222222222222222222222222222222222';
const NONCE16 = '000102030405060708090a0b0c0d0e0f';
const NONCE12 = '000102030405060708090a0b';
const PAYLOAD_HEX =
  '04466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f276728176c3c6431f8eeda4538dc37c865e2784f3a9e77d044f33e407797e1278a000102030405060708090a0b0c0d0e0f52a1afe24dfe7e9300a0def3bdde58599c7c7ab7575286def1cdb7518a35';
const PAYLOAD_B64 =
  'BEZtf8rlY+XLCaDRhwu1gDRIBGF4eaFJSc8iKF8brj8nZygXbDxkMfju2kU43DfIZeJ4Tzqed9BE8z5Ad5fhJ4oAAQIDBAUGBwgJCgsMDQ4PUqGv4k3+fpMAoN7zvd5YWZx8erdXUobe8c23UYo1';
const UNCOMPRESSED_12_HEX =
  '04466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f276728176c3c6431f8eeda4538dc37c865e2784f3a9e77d044f33e407797e1278a000102030405060708090a0b0aaf7a5c677ffbc9f59a76871def2e224ee3f5b759efcda23132f89b7513';
const COMPRESSED_HEX =
  '02466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f27000102030405060708090a0b0aaf7a5c677ffbc9f59a76871def2e224ee3f5b759efcda23132f89b7513';

async function outputText(page: Page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

async function fillDeterministicEncrypt(page: Page, outputEncoding = 'hex') {
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-data', 'attack at dawn');
  await page.fill('#in-key', RECIPIENT_PUB);
  await page.selectOption('#in-curve', 'secp256k1');
  await page.selectOption('#in-cipher', 'aes-256-gcm');
  await page.selectOption('#in-nonce_length', '16');
  await page.fill('#in-nonce', NONCE16);
  await page.selectOption('#in-kdf_input', 'ephemeral-and-point');
  await page.fill('#in-ephemeral_key', EPHEMERAL_SK);
  await page.selectOption('#in-key_encoding', 'auto');
  await page.selectOption('#in-data_encoding', 'auto');
  await page.selectOption('#in-output_encoding', outputEncoding);
}

test('ecies-encrypt page — encrypts exact deterministic hex payload', async ({ page }) => {
  await page.goto('/tools/ecies-encrypt/');
  await fillDeterministicEncrypt(page, 'hex');
  await expect(page.locator('#tool-output')).toContainText(PAYLOAD_HEX, { timeout: 15000 });
  expect(await outputText(page)).toBe(PAYLOAD_HEX);
});

test('ecies-encrypt page — decrypts combined base64 payload', async ({ page }) => {
  await page.goto('/tools/ecies-encrypt/');
  await page.selectOption('#in-operation', 'decrypt');
  await page.fill('#in-data', PAYLOAD_B64);
  await page.fill('#in-key', RECIPIENT_SK);
  await page.selectOption('#in-curve', 'secp256k1');
  await page.selectOption('#in-cipher', 'aes-256-gcm');
  await page.selectOption('#in-nonce_length', '16');
  await page.selectOption('#in-kdf_input', 'ephemeral-and-point');
  await page.selectOption('#in-key_encoding', 'auto');
  await page.selectOption('#in-data_encoding', 'auto');
  await page.selectOption('#in-output_encoding', 'base64');
  await expect(page.locator('#tool-output')).toContainText('attack at dawn', { timeout: 15000 });
  expect(await outputText(page)).toBe('attack at dawn');
});

test('ecies-encrypt page — query-param deep-link 12-byte nonce hex output', async ({ page }) => {
  await page.goto(
    '/tools/ecies-encrypt/?operation=encrypt&data=' +
      encodeURIComponent('attack at dawn') +
      '&key=' +
      RECIPIENT_PUB +
      '&curve=secp256k1&cipher=aes-256-gcm&nonce_length=12&nonce=' +
      NONCE12 +
      '&compressed_ephemeral=false&kdf_input=ephemeral-and-point&ephemeral_key=' +
      EPHEMERAL_SK +
      '&key_encoding=auto&data_encoding=auto&output_encoding=hex',
  );
  await expect(page.locator('#in-operation')).toHaveValue('encrypt', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText(UNCOMPRESSED_12_HEX, { timeout: 15000 });
  expect(await outputText(page)).toBe(UNCOMPRESSED_12_HEX);
});

test('ecies-encrypt page — compressed ephemeral checkbox changes payload layout', async ({ page }) => {
  await page.goto('/tools/ecies-encrypt/');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-data', 'attack at dawn');
  await page.fill('#in-key', RECIPIENT_PUB);
  await page.selectOption('#in-curve', 'secp256k1');
  await page.selectOption('#in-cipher', 'aes-256-gcm');
  await page.selectOption('#in-nonce_length', '12');
  await page.fill('#in-nonce', NONCE12);
  await page.check('#in-compressed_ephemeral');
  await page.selectOption('#in-kdf_input', 'ephemeral-and-point');
  await page.fill('#in-ephemeral_key', EPHEMERAL_SK);
  await page.selectOption('#in-output_encoding', 'hex');
  await expect(page.locator('#tool-output')).toContainText(COMPRESSED_HEX, { timeout: 15000 });
  expect(await outputText(page)).toBe(COMPRESSED_HEX);
});

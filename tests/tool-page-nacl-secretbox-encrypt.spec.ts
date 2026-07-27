import { test, expect } from './fixtures';

const KEY = '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff';
const NONCE = '000102030405060708090a0b0c0d0e0f1011121314151617';
const BOX_B64 = 'AAECAwQFBgcICQoLDA0ODxAREhMUFRYXRW0mB2zfUpMbVb9qt3nRcUCZ66X999LGingCchiB';
const BOX_HEX = '000102030405060708090a0b0c0d0e0f1011121314151617456d26076cdf52931b55bf6ab779d1714099eba5fdf7d2c68a7802721881';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('nacl-secretbox-encrypt page — encrypts exact base64 combined box', async ({ page }) => {
  await page.goto('/tools/nacl-secretbox-encrypt/');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-data', 'attack at dawn');
  await page.fill('#in-key', KEY);
  await page.fill('#in-nonce', NONCE);
  await page.selectOption('#in-key_encoding', 'hex');
  await page.selectOption('#in-nonce_encoding', 'hex');
  await page.selectOption('#in-data_encoding', 'text');
  await page.selectOption('#in-output_encoding', 'base64');
  await expect(page.locator('#tool-output')).toContainText(BOX_B64, { timeout: 15000 });
  expect(await outputText(page)).toBe(BOX_B64);
});

test('nacl-secretbox-encrypt page — decrypts combined box', async ({ page }) => {
  await page.goto('/tools/nacl-secretbox-encrypt/');
  await page.selectOption('#in-operation', 'decrypt');
  await page.fill('#in-data', BOX_B64);
  await page.fill('#in-key', KEY);
  await page.selectOption('#in-key_encoding', 'hex');
  await page.selectOption('#in-data_encoding', 'base64');
  await page.selectOption('#in-output_encoding', 'base64');
  await expect(page.locator('#tool-output')).toContainText('attack at dawn', { timeout: 15000 });
  expect(await outputText(page)).toBe('attack at dawn');
});

test('nacl-secretbox-encrypt page — query-param deep-link hex output', async ({ page }) => {
  await page.goto(
    '/tools/nacl-secretbox-encrypt/?operation=encrypt&data=' +
      encodeURIComponent('attack at dawn') +
      '&key=' +
      KEY +
      '&nonce=' +
      NONCE +
      '&key_encoding=hex&nonce_encoding=hex&data_encoding=text&output_encoding=hex',
  );
  await expect(page.locator('#in-operation')).toHaveValue('encrypt', { timeout: 15000 });
  await expect(page.locator('#in-output_encoding')).toHaveValue('hex');
  await expect(page.locator('#tool-output')).toContainText(BOX_HEX, { timeout: 15000 });
  expect(await outputText(page)).toBe(BOX_HEX);
});

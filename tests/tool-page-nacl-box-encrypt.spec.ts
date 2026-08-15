import { test, expect } from './fixtures';

const ALICE_SECRET = '77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a';
const ALICE_PUBLIC = '8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a';
const BOB_SECRET = '5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb';
const BOB_PUBLIC = 'de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f';
const NONCE = '000102030405060708090a0b0c0d0e0f1011121314151617';
const BOX_HEX =
  '000102030405060708090a0b0c0d0e0f1011121314151617f6a8999dbcb32c653a4387c1367a33da643a24c4781d751c34a7fc1fd4e4';
const BOX_B64 = 'AAECAwQFBgcICQoLDA0ODxAREhMUFRYX9qiZnbyzLGU6Q4fBNnoz2mQ6JMR4HXUcNKf8H9Tk';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

test('nacl-box-encrypt page — encrypts exact hex combined box', async ({ page }) => {
  await page.goto('/tools/nacl-box-encrypt/');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-data', 'attack at dawn');
  await page.fill('#in-recipient_key', BOB_PUBLIC);
  await page.fill('#in-sender_key', ALICE_SECRET);
  await page.fill('#in-nonce', NONCE);
  await page.selectOption('#in-key_encoding', 'hex');
  await page.selectOption('#in-nonce_encoding', 'hex');
  await page.selectOption('#in-data_encoding', 'text');
  await page.selectOption('#in-output_encoding', 'hex');
  await expect(page.locator('#tool-output')).toContainText(BOX_HEX, { timeout: 15000 });
  expect(await outputText(page)).toBe(BOX_HEX);
});

test('nacl-box-encrypt page — decrypts combined base64 box', async ({ page }) => {
  await page.goto('/tools/nacl-box-encrypt/');
  await page.selectOption('#in-operation', 'decrypt');
  await page.fill('#in-data', BOX_B64);
  await page.fill('#in-recipient_key', BOB_SECRET);
  await page.fill('#in-sender_key', ALICE_PUBLIC);
  await page.selectOption('#in-key_encoding', 'hex');
  await page.selectOption('#in-data_encoding', 'base64');
  await page.selectOption('#in-output_encoding', 'base64');
  await expect(page.locator('#tool-output')).toContainText('attack at dawn', { timeout: 15000 });
  expect(await outputText(page)).toBe('attack at dawn');
});

test('nacl-box-encrypt page — query-param deep-link hex output', async ({ page }) => {
  await page.goto(
    '/tools/nacl-box-encrypt/?operation=encrypt&data=' +
      encodeURIComponent('attack at dawn') +
      '&recipient_key=' +
      BOB_PUBLIC +
      '&sender_key=' +
      ALICE_SECRET +
      '&nonce=' +
      NONCE +
      '&key_encoding=hex&nonce_encoding=hex&data_encoding=text&output_encoding=hex',
  );
  await expect(page.locator('#in-operation')).toHaveValue('encrypt', { timeout: 15000 });
  await expect(page.locator('#in-output_encoding')).toHaveValue('hex');
  await expect(page.locator('#tool-output')).toContainText(BOX_HEX, { timeout: 15000 });
  expect(await outputText(page)).toBe(BOX_HEX);
});

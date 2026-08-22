import { test, expect } from './fixtures';

// Fixed vector from blocks/scryptenc-file/core/src/lib.rs and page/meta.toml.
const VECTOR_HEX =
  '736372797074000a0000000800000001000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1fda46ceb5d5738b6fc865e137d56ab5898f39c2cf6c77fbc8a950f80b58a7c22e3b90cd9663c66cd5fadb6557a17ca0cc5d0aae767789f0fe3f4e1eb6298b40ec5c12b45666e4c0bcb195782c41eacbc3ebcce48dad';
const VECTOR_B64 =
  'c2NyeXB0AAoAAAAIAAAAAQABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4f2kbOtdVzi2/IZeE31Wq1iY85ws9sd/vIqVD4C1inwi47kM2WY8Zs1frbZVehfKDMXQqudneJ8P4/Th62KYtA7FwStFZm5MC8sZV4LEHqy8PrzOSNrQ==';
const SALT_HEX = '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f';

test('scryptenc-file page encrypts the fixed vector exactly', async ({ page }) => {
  await page.goto('/tools/scryptenc-file/');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-data', 'hello');
  await page.fill('#in-password', 'pleaseletmein');
  await page.selectOption('#in-data_encoding', 'text');
  await page.selectOption('#in-output_encoding', 'hex');
  await page.fill('#in-log_n', '10');
  await page.fill('#in-r', '8');
  await page.fill('#in-p', '1');
  await page.fill('#in-salt', SALT_HEX);
  await page.fill('#in-max_memory_mib', '32');

  await expect(page.locator('#tool-output')).toHaveText(VECTOR_HEX, { timeout: 15_000 });
});

test('scryptenc-file page decrypts base64 and reports header info', async ({ page }) => {
  await page.goto('/tools/scryptenc-file/');
  await page.selectOption('#in-operation', 'decrypt');
  await page.fill('#in-data', VECTOR_B64);
  await page.fill('#in-password', 'pleaseletmein');
  await page.selectOption('#in-data_encoding', 'base64');
  await page.selectOption('#in-output_encoding', 'base64');
  await page.fill('#in-log_n', '14');
  await page.fill('#in-r', '8');
  await page.fill('#in-p', '1');
  await page.fill('#in-salt', '');
  await page.fill('#in-max_memory_mib', '32');

  await expect(page.locator('#tool-output')).toHaveText('hello', { timeout: 15_000 });

  await page.selectOption('#in-operation', 'info');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('format: scrypt encrypted data', { timeout: 15_000 });
  await expect(out).toContainText('logN: 10');
  await expect(out).toContainText(`salt: ${SALT_HEX}`);
  await expect(out).toContainText('ciphertext: 5 bytes');
});

test('scryptenc-file deep link prefills params and computes', async ({ page }) => {
  const params = new URLSearchParams({
    operation: 'info',
    data: VECTOR_HEX,
    password: '',
    data_encoding: 'hex',
    output_encoding: 'base64',
    log_n: '14',
    r: '8',
    p: '1',
    salt: '',
    max_memory_mib: '32',
  });
  await page.goto(`/tools/scryptenc-file/?${params.toString()}`);

  await expect(page.locator('#in-operation')).toHaveValue('info', { timeout: 15_000 });
  await expect(page.locator('#in-data')).toHaveValue(VECTOR_HEX);
  await expect(page.locator('#in-data_encoding')).toHaveValue('hex');
  await expect(page.locator('#in-output_encoding')).toHaveValue('base64');
  await expect(page.locator('#in-max_memory_mib')).toHaveValue('32');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('format: scrypt encrypted data', { timeout: 15_000 });
  await expect(out).toContainText('total: 133 bytes');
});

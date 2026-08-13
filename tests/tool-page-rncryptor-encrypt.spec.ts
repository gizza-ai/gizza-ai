import { test, expect } from './fixtures';

const VECTOR = '03010001020304050607010203040506070802030405060708090a0b0c0d0e0f0001a1f8730e0bf480eb7b70f690abf21e029514164ad3c474a51b30c7eaa1ca545b7de3de5b010acbad0a9a13857df696a8';

async function runWasm(
  page: any,
  operation = 'encrypt',
  data = '01',
  password = 'thepassword',
  data_encoding = 'hex',
  output_encoding = 'hex',
  encryption_salt = '0001020304050607',
  hmac_salt = '0102030405060708',
  iv = '02030405060708090a0b0c0d0e0f0001',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/rncryptor-encrypt/gizza_ai_rncryptor_encrypt_web.js');
    await mod.default('/tools/rncryptor-encrypt/gizza_ai_rncryptor_encrypt_web_bg.wasm');
    return mod.run(
      args.operation,
      args.data,
      args.password,
      args.data_encoding,
      args.output_encoding,
      args.encryption_salt,
      args.hmac_salt,
      args.iv,
    );
  }, { operation, data, password, data_encoding, output_encoding, encryption_salt, hmac_salt, iv });
}

test('rncryptor-encrypt wasm reproduces the deterministic v3 spec vector and decrypts it', async ({ page }) => {
  await page.goto('/tools/rncryptor-encrypt/');
  await page.waitForSelector('#in-data');

  const encrypted = await runWasm(page);
  expect(encrypted).toBe(VECTOR);

  const decrypted = await runWasm(page, 'decrypt', encrypted, 'thepassword', 'hex', 'hex', '', '', '');
  expect(decrypted).toBe('01');

  await expect(runWasm(page, 'decrypt', encrypted, 'wrong', 'hex', 'hex', '', '', ''))
    .rejects.toThrow(/HMAC verification failed/);
});

test('rncryptor-encrypt page computes exact hex output from the form', async ({ page }) => {
  await page.goto('/tools/rncryptor-encrypt/');
  await page.selectOption('#in-operation', 'encrypt');
  await page.fill('#in-data', '01');
  await page.fill('#in-password', 'thepassword');
  await page.selectOption('#in-data_encoding', 'hex');
  await page.selectOption('#in-output_encoding', 'hex');
  await page.fill('#in-encryption_salt', '0001020304050607');
  await page.fill('#in-hmac_salt', '0102030405060708');
  await page.fill('#in-iv', '02030405060708090a0b0c0d0e0f0001');

  await expect(page.locator('#tool-output')).toHaveText(VECTOR, { timeout: 15_000 });
});

test('rncryptor-encrypt deep link covers decrypt, base64 input, and text output', async ({ page }) => {
  const base64Container = 'AwEAAQIDBAUGBwECAwQFBgcIAgMEBQYHCAkKCwwNDg8AAaH4cw4L9IDre3D2kKvyHgKVFBZK08R0pRswx+qhylRbfePeWwEKy60KmhOFffaWqA==';
  const params = new URLSearchParams({
    operation: 'decrypt',
    data: base64Container,
    password: 'thepassword',
    data_encoding: 'base64',
    output_encoding: 'hex',
    encryption_salt: '',
    hmac_salt: '',
    iv: '',
  });
  await page.goto(`/tools/rncryptor-encrypt/?${params.toString()}`);

  await expect(page.locator('#in-operation')).toHaveValue('decrypt', { timeout: 15_000 });
  await expect(page.locator('#in-data_encoding')).toHaveValue('base64');
  await expect(page.locator('#in-output_encoding')).toHaveValue('hex');
  await expect(page.locator('#tool-output')).toHaveText('01', { timeout: 15_000 });
});

test('rncryptor-encrypt generated CLI example is generic and parseable', async ({ page }) => {
  await page.goto('/tools/rncryptor-encrypt/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool rncryptor-encrypt');
  expect(cli).toContain('operation=encrypt');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

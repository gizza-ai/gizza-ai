import { test, expect } from './fixtures';

const YAML = `app: demo
database:
  host_unencrypted: db.internal
  password: s3cr3t
  port: 5432
`;
const PASSPHRASE = 'correct horse battery staple';

async function runWasm(
  page,
  document = YAML,
  passphrase = PASSPHRASE,
  mode = 'encrypt',
  format = 'auto',
  encrypted_suffix = '',
  unencrypted_suffix = '_unencrypted',
  encrypted_regex = '',
  unencrypted_regex = '',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/sops-encrypt/gizza_ai_sops_encrypt_web.js');
    await mod.default('/tools/sops-encrypt/gizza_ai_sops_encrypt_web_bg.wasm');
    return mod.run(
      args.document,
      args.passphrase,
      args.mode,
      args.format,
      args.encrypted_suffix,
      args.unencrypted_suffix,
      args.encrypted_regex,
      args.unencrypted_regex,
    );
  }, { document, passphrase, mode, format, encrypted_suffix, unencrypted_suffix, encrypted_regex, unencrypted_regex });
}

function expectEncryptedYaml(out: string) {
  expect(out).toContain('app: ENC[GZAE1,');
  expect(out).toContain('password: ENC[GZAE1,');
  expect(out).toContain('port: ENC[GZAE1,');
  expect(out).toContain('host_unencrypted: db.internal');
  expect(out).toContain('gizza_sops:');
  expect(out).not.toContain('s3cr3t');
  expect(out).not.toContain('5432');
}

test('sops-encrypt wasm encrypts values and decrypts back to cleartext', async ({ page }) => {
  await page.goto('/tools/sops-encrypt/');
  await page.waitForSelector('#in-document');

  const encrypted = await runWasm(page);
  expectEncryptedYaml(encrypted);

  const decrypted = await runWasm(page, encrypted, PASSPHRASE, 'decrypt');
  expect(decrypted).toContain('app: demo');
  expect(decrypted).toContain('host_unencrypted: db.internal');
  expect(decrypted).toContain('password: s3cr3t');
  expect(decrypted).toContain('port: 5432');
  expect(decrypted).not.toContain('gizza_sops');
});

test('sops-encrypt wasm covers format enum and key-selection rules', async ({ page }) => {
  await page.goto('/tools/sops-encrypt/');
  await page.waitForSelector('#in-document');

  const json = '{"host":"api.internal","password":"s3cr3t","token":"t-9000","replicas":3}';
  const onlySecrets = await runWasm(page, json, PASSPHRASE, 'encrypt', 'json', '', '', '^(password|token)$', '');
  const parsed = JSON.parse(onlySecrets);
  expect(parsed.host).toBe('api.internal');
  expect(parsed.replicas).toBe(3);
  expect(parsed.password).toMatch(/^ENC\[GZAE1,/);
  expect(parsed.token).toMatch(/^ENC\[GZAE1,/);

  const env = '# service config\nAPI_KEY=abc123\nexport LOG_LEVEL=debug\n';
  const envEncrypted = await runWasm(page, env, PASSPHRASE, 'encrypt', 'env', '', '', '', '^LOG_LEVEL$');
  expect(envEncrypted).toContain('# service config');
  expect(envEncrypted).toContain('API_KEY=ENC[GZAE1,');
  expect(envEncrypted).toContain('export LOG_LEVEL=debug');
  expect(envEncrypted).toContain('GIZZA_SOPS_SALT=');

  await expect(runWasm(page, YAML, PASSPHRASE, 'encrypt', 'yaml', '_secret', '_clear'))
    .rejects.toThrow(/only one key-selection rule/);
});

test('sops-encrypt page encrypts from the form and decrypts the output', async ({ page }) => {
  await page.goto('/tools/sops-encrypt/');
  await page.fill('#in-document', YAML);
  await page.fill('#in-passphrase', PASSPHRASE);
  await page.selectOption('#in-format', 'yaml');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('password: ENC[GZAE1,', { timeout: 15_000 });
  await expect(out).toContainText('host_unencrypted: db.internal');
  await expect(out).not.toContainText('s3cr3t');

  const encrypted = await out.innerText();
  await page.selectOption('#in-mode', 'decrypt');
  await page.fill('#in-document', encrypted);
  await expect(out).toContainText('password: s3cr3t', { timeout: 15_000 });
  await expect(out).toContainText('port: 5432');
  await expect(out).not.toContainText('gizza_sops');
});

test('sops-encrypt page pre-fills and computes from deep links', async ({ page }) => {
  const qs = new URLSearchParams({
    document: '{"host":"api.internal","password":"s3cr3t","token":"t-9000"}',
    passphrase: PASSPHRASE,
    mode: 'encrypt',
    format: 'json',
    encrypted_regex: '^(password|token)$',
    unencrypted_suffix: '',
  });
  await page.goto(`/tools/sops-encrypt/?${qs.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('json', { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('encrypt');
  await expect(page.locator('#in-encrypted_regex')).toHaveValue('^(password|token)$');
  await expect(page.locator('#in-unencrypted_suffix')).toHaveValue('');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"host": "api.internal"', { timeout: 15_000 });
  await expect(out).toContainText('"password": "ENC[GZAE1,');
  await expect(out).toContainText('"token": "ENC[GZAE1,');
});

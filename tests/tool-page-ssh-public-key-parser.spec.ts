import { test, expect } from './fixtures';

const ED25519 = 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPc21YeL9wdmn0Bvy1dVCZH/rO/hcbVFBt5YQ/Y8+oOy alice@example.com';
const RSA1024 = 'ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAAAgQDekXDjgNS3ZxCX3k9Vy2bXyMehgtLFWjqnh/lDhATJjzF97/zmGaPA/4qCyYd5dzfRkwwjldgT+SSbsiPIcREcQKWAF9/jzi5OQh2jOmcaCtUpPs8yRQTBngXobCX2DZx69ZRW01iKRHqWMdKRuyaTGBC4OqQ803k5hIcAuo6KTQ== weak@old';

async function outputJson(page) {
  const text = await page.locator('#tool-output').textContent({ timeout: 20000 });
  return JSON.parse(text ?? '');
}

test('ssh-public-key-parser reports exact fingerprints for an Ed25519 public key', async ({ page }) => {
  await page.goto('/tools/ssh-public-key-parser/');
  await page.fill('#in-input', ED25519);

  const report = await outputJson(page);
  expect(report.key_count).toBe(1);
  expect(report.unique_fingerprints).toBe(1);
  expect(report.keys[0].algorithm).toBe('ssh-ed25519');
  expect(report.keys[0].key_type).toBe('Ed25519');
  expect(report.keys[0].key_size_bits).toBe(256);
  expect(report.keys[0].comment).toBe('alice@example.com');
  expect(report.keys[0].fingerprint_sha256).toBe('SHA256:/PcooB4wsFrX/EAwN1wlE0KJbNvM1usU1KT6lCXUah4');
  expect(report.keys[0].fingerprint_md5).toBe('MD5:1e:e5:90:86:13:ab:0e:5a:24:3a:30:5d:7b:53:e3:fe');
  expect(report.keys[0].strength).toBe('strong');
});

test('ssh-public-key-parser applies a deep-link fingerprint comparison', async ({ page }) => {
  const qs =
    '?input=' + encodeURIComponent(ED25519) +
    '&expected_fingerprint=' + encodeURIComponent('SHA256:/PcooB4wsFrX/EAwN1wlE0KJbNvM1usU1KT6lCXUah4') +
    '&include_sha1=true' +
    '&uppercase_md5=true';

  await page.goto('/tools/ssh-public-key-parser/' + qs);
  await expect(page.locator('#in-include_sha1')).toBeChecked({ timeout: 15000 });
  await expect(page.locator('#in-uppercase_md5')).toBeChecked();

  const report = await outputJson(page);
  expect(report.expected_fingerprint_matched).toBe(true);
  expect(report.keys[0].fingerprint_match).toBe(true);
  expect(report.keys[0].fingerprint_sha1).toBe('SHA1:wu8NvLyw+V/NWTfudrcG9O7ImtI');
  expect(report.keys[0].fingerprint_md5).toBe('MD5:1E:E5:90:86:13:AB:0E:5A:24:3A:30:5D:7B:53:E3:FE');
});

test('ssh-public-key-parser warns about weak RSA keys', async ({ page }) => {
  await page.goto('/tools/ssh-public-key-parser/');
  await page.fill('#in-input', RSA1024);

  const report = await outputJson(page);
  expect(report.keys[0].key_type).toBe('RSA');
  expect(report.keys[0].key_size_bits).toBe(1024);
  expect(report.keys[0].strength).toBe('weak');
  expect(report.keys[0].warnings.join('\n')).toContain('2048-bit minimum');
});

test('ssh-public-key-parser rejects private keys clearly', async ({ page }) => {
  await page.goto('/tools/ssh-public-key-parser/');
  await page.fill('#in-input', '-----BEGIN OPENSSH PRIVATE KEY-----\nnot-a-real-key\n-----END OPENSSH PRIVATE KEY-----');

  await expect(page.locator('#tool-output')).toHaveClass(/error/, { timeout: 20000 });
  await expect(page.locator('#tool-output')).toContainText('PRIVATE key');
  await expect(page.locator('#tool-output')).toContainText('matching public key');
});

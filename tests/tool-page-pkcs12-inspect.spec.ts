import { test, expect } from './fixtures';
import * as fs from 'fs';
import * as path from 'path';

// Same fixture the Rust unit tests use: a self-signed P-256 certificate plus its
// key, exported with UNENCRYPTED bags so the whole structure is readable.
const B64 = fs
  .readFileSync(
    path.resolve(__dirname, '../blocks/pkcs12-inspect/core/tests/fixtures/ec-plain.p12.b64'),
    'utf8',
  )
  .trim();

const HEX = Buffer.from(B64, 'base64').toString('hex');

test('pkcs12-inspect page lists bags, friendly names and certificate details', async ({ page }) => {
  await page.goto('/tools/pkcs12-inspect/');
  await page.fill('#in-data', B64);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('PKCS#12 container: version 3, 967 bytes', { timeout: 15000 });
  const text = (await out.textContent()) ?? '';
  expect(text).toContain(
    'Integrity MAC: SHA-256, 2048 iterations (MAC 32 bytes, salt 8 bytes) — not verified (needs the password)',
  );
  expect(text).toContain('Bag 1: certBag (1.2.840.113549.1.12.10.1.3)');
  expect(text).toContain('friendlyName: EC Sample');
  expect(text).toContain('localKeyID: 45F5FCB2E17ECF8E9E95E91CE1FFE1733A8E4A56');
  expect(text).toContain('subject: C=US, O=Gizza Test, CN=ec.example.test');
  expect(text).toContain('public key: EC 256 bit');
  expect(text).toContain('signature: ECDSA with SHA-256');
  expect(text).toContain(
    'SHA-256: 07:D6:B0:AD:0A:17:10:FC:59:7F:29:BD:4C:06:99:D7:2D:D5:4C:FE:CC:5B:19:31:64:1E:4C:CD:15:ED:33:9B',
  );
  // The key bag pairs with the cert bag via the same localKeyID.
  expect(text).toContain('Bag 1: keyBag (1.2.840.113549.1.12.10.1.1)');
});

test('pkcs12-inspect deep-link renders JSON output', async ({ page }) => {
  await page.goto(
    `/tools/pkcs12-inspect/?data=${encodeURIComponent(B64)}&format=json`,
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"bag_type": "certBag"', { timeout: 15000 });
  const text = (await out.textContent()) ?? '';
  expect(text).toContain('"friendly_name": "EC Sample"');
  expect(text).toContain('"certificate_bags": 1');
  expect(text).toContain('"password_required": false');
});

test('pkcs12-inspect accepts hex input when the encoding select says hex', async ({ page }) => {
  await page.goto('/tools/pkcs12-inspect/');
  await page.selectOption('#in-encoding', 'hex');
  await page.fill('#in-data', HEX);
  await expect(page.locator('#tool-output')).toContainText(
    'subject: C=US, O=Gizza Test, CN=ec.example.test',
    { timeout: 15000 },
  );
});

test('pkcs12-inspect reports encryption parameters of a protected keystore', async ({ page }) => {
  const aes = fs
    .readFileSync(
      path.resolve(__dirname, '../blocks/pkcs12-inspect/core/tests/fixtures/default-aes.p12.b64'),
      'utf8',
    )
    .trim();
  await page.goto('/tools/pkcs12-inspect/');
  await page.fill('#in-data', aes);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Password required to extract: yes', { timeout: 15000 });
  const text = (await out.textContent()) ?? '';
  expect(text).toContain('Encryption: PBES2, PBKDF2, AES-256-CBC, 2048 iterations, PRF hmacWithSHA256');
  expect(text).toContain('Bag 1: pkcs8ShroudedKeyBag (1.2.840.113549.1.12.10.1.2)');
});

test('pkcs12-inspect rejects input that is not a PKCS#12 container', async ({ page }) => {
  await page.goto('/tools/pkcs12-inspect/?data=aGVsbG8gd29ybGQgbm90IGEgcDEy');
  await expect(page.locator('#tool-error, #tool-output')).toContainText(
    'not a PKCS#12 container',
    { timeout: 15000 },
  );
});

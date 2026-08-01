import { readFileSync } from 'node:fs';
import path from 'node:path';
import { test, expect } from './fixtures';

const CERT = readFileSync(
  path.resolve(__dirname, '../blocks/pem-inspect/core/tests/fixtures/cert.pem'),
  'utf8',
);
const PUB = readFileSync(
  path.resolve(__dirname, '../blocks/pem-inspect/core/tests/fixtures/pub.pem'),
  'utf8',
);

async function outputText(page): Promise<string> {
  const text = await page.locator('#tool-output').textContent();
  return (text ?? '').replace(/\s+$/, '');
}

test('pem-inspect page decodes a certificate with real fields', async ({ page }) => {
  await page.goto('/tools/pem-inspect/');
  await page.fill('#in-input', CERT);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"type": "certificate"', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('"subject": "CN=pem-inspect.example, O=Gizza Toolkit, C=US"');
  expect(text).toContain('"issuer": "CN=pem-inspect.example, O=Gizza Toolkit, C=US"');
  expect(text).toContain('"algorithm": "RSA"');
  expect(text).toContain('"key_size_bits": 2048');
  expect(text).toContain('"fingerprint_sha256": "29:63:F1:CF:05:8E:09:3A:D6:E0:78:69:42:BF:BD:9D:11:BF:40:85:F9:D0:C2:5C:9C:F2:18:D3:DF:B2:A4:66"');
});

test('pem-inspect page supports multiple PEM blocks', async ({ page }) => {
  await page.goto('/tools/pem-inspect/');
  await page.fill('#in-input', `${CERT}\n${PUB}`);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"type": "certificate"', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('"type": "public_key"');
  expect(text).toContain('"format": "SubjectPublicKeyInfo (SPKI)"');
});

test('pem-inspect page deep-link prefills and decodes', async ({ page }) => {
  await page.goto('/tools/pem-inspect/?input=' + encodeURIComponent(PUB));

  await expect(page.locator('#in-input')).toHaveValue(PUB, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"type": "public_key"', { timeout: 15000 });
  const text = await outputText(page);
  expect(text).toContain('"algorithm": "RSA"');
  expect(text).toContain('"key_size_bits": 2048');
});

import { test, expect } from './fixtures';

const BUNDLE = '-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----\n-----BEGIN CERTIFICATE REQUEST-----\nBAUG\n-----END CERTIFICATE REQUEST-----';

const REPORT = `PEM bundle: 2 blocks
  certificate: 1 · certificate signing request: 1

Block 1 of 2
  type:     X.509 certificate
  label:    CERTIFICATE
  DER size: 3 bytes
  filename: block-1-certificate.pem

-----BEGIN CERTIFICATE-----
AQID
-----END CERTIFICATE-----

Block 2 of 2
  type:     PKCS#10 certificate signing request (CSR)
  label:    CERTIFICATE REQUEST
  DER size: 3 bytes
  filename: block-2-certificate-request.pem

-----BEGIN CERTIFICATE REQUEST-----
BAUG
-----END CERTIFICATE REQUEST-----`;

const PEM_OUTPUT = `# Block 1 of 2: X.509 certificate (block-1-certificate.pem)
-----BEGIN CERTIFICATE-----
AQID
-----END CERTIFICATE-----

# Block 2 of 2: PKCS#10 certificate signing request (CSR) (block-2-certificate-request.pem)
-----BEGIN CERTIFICATE REQUEST-----
BAUG
-----END CERTIFICATE REQUEST-----`;

test('pem-bundle-splitter renders an exact report without fingerprints', async ({ page }) => {
  await page.goto('/tools/pem-bundle-splitter/');
  await page.fill('#in-pem', BUNDLE);
  await page.selectOption('#in-output', 'report');
  await page.uncheck('#in-fingerprints');

  await expect(page.locator('#tool-output')).toHaveText(REPORT, { timeout: 15000 });
});

test('pem-bundle-splitter emits clean PEM blocks', async ({ page }) => {
  await page.goto('/tools/pem-bundle-splitter/');
  await page.fill('#in-pem', BUNDLE);
  await page.selectOption('#in-output', 'pem');
  await page.uncheck('#in-fingerprints');

  await expect(page.locator('#tool-output')).toHaveText(PEM_OUTPUT, { timeout: 15000 });
});

test('pem-bundle-splitter deep-link pre-fills params and auto-runs JSON', async ({ page }) => {
  const params = new URLSearchParams({
    pem: '-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----',
    output: 'json',
    fingerprints: 'true',
  });

  await page.goto(`/tools/pem-bundle-splitter/?${params.toString()}`);
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#in-fingerprints')).toBeChecked();
  await expect(page.locator('#tool-output')).toContainText('"sha256": "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('"type": "X.509 certificate"');
});

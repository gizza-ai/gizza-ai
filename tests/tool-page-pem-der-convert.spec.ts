import { test, expect } from './fixtures';

// /tools/pem-der-convert/ converts PEM<->DER in-browser (pure wasm).

test('pem-der-convert page converts PEM to DER hex', async ({ page }) => {
  await page.goto('/tools/pem-der-convert/');
  await page.fill(
    '#in-input',
    '-----BEGIN CERTIFICATE-----\nAAECAwQFBgcICQ==\n-----END CERTIFICATE-----'
  );
  await page.selectOption('#in-direction', 'pem-to-der');
  await page.selectOption('#in-der_format', 'hex');
  await expect(page.locator('#tool-output')).toContainText('00010203040506070809', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('label: CERTIFICATE', {
    timeout: 15000,
  });
});

test('pem-der-convert page wraps DER into PEM via deep-link', async ({ page }) => {
  const qs =
    '?input=00010203040506070809&direction=der-to-pem&der_format=hex&label=' +
    encodeURIComponent('EC PRIVATE KEY');
  await page.goto('/tools/pem-der-convert/' + qs);
  await expect(page.locator('#tool-output')).toContainText('-----BEGIN EC PRIVATE KEY-----', {
    timeout: 15000,
  });
});

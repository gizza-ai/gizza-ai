import { test, expect } from './fixtures';

const samplePayload =
  'CikKBUhlbGxvEhFhbGljZUBleGFtcGxlLmNvbRoHRXhhbXBsZSABKAEwAgoaCgVXb3JsZBIDYm9iGgRBQ01FIAIoAjABOAUQAQ';

const expectedUris =
  'otpauth://totp/Example:alice%40example.com?secret=JBSWY3DP&issuer=Example&algorithm=SHA1&digits=6&period=30\n' +
  'otpauth://hotp/ACME:bob?secret=K5XXE3DE&issuer=ACME&algorithm=SHA256&digits=8&counter=5';

test('otpauth-migration-decoder renders standard otpauth URIs', async ({ page }) => {
  await page.goto('/tools/otpauth-migration-decoder/');
  await page.fill('#in-payload', samplePayload);
  await expect(page.locator('#tool-output')).toHaveText(expectedUris, {
    timeout: 15000,
  });
});

test('otpauth-migration-decoder JSON format exposes account fields', async ({ page }) => {
  await page.goto('/tools/otpauth-migration-decoder/');
  await page.fill('#in-payload', `otpauth-migration://offline?data=${samplePayload}`);
  await page.selectOption('#in-format', 'json');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('"issuer": "Example"', { timeout: 15000 });
  await expect(output).toContainText('"secret": "JBSWY3DP"');
  await expect(output).toContainText('"counter": 5');
  await expect(output).toContainText('otpauth://hotp/ACME:bob');
});

test('otpauth-migration-decoder query-param deep-link prefills controls', async ({ page }) => {
  await page.goto(
    '/tools/otpauth-migration-decoder/?payload=' +
      encodeURIComponent(samplePayload) +
      '&format=json',
  );
  await expect(page.locator('#in-payload')).toHaveValue(samplePayload, {
    timeout: 15000,
  });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"type": "totp"', {
    timeout: 15000,
  });
});

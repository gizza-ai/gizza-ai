import { test, expect } from './fixtures';

// /tools/pem-public-key-extract/ derives the SPKI public key from a private
// key, entirely in-browser (pure wasm). Fixtures generated with openssl and
// cross-checked against `openssl pkey -pubout`.

const EC_PRIV =
  '-----BEGIN EC PRIVATE KEY-----\n' +
  'MHcCAQEEIKA38QRXEBpjLtNMjXRJpd8uVk4ro9CP3fRW5scu/6spoAoGCCqGSM49\n' +
  'AwEHoUQDQgAEbRrVl21CWYJFvoLnQXItdvpGkjd0X4UxXDkCetbH/m+EjhA4x0bo\n' +
  'iAM255xx2K/Zg9xPmujdbNM3RvBOlf0W0w==\n' +
  '-----END EC PRIVATE KEY-----';

// The matching public key (openssl pkey -pubout), first base64 line is stable.
const EC_PUB_LINE = 'MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEbRrVl21CWYJFvoLnQXItdvpGkjd0';

test('pem-public-key-extract page derives EC public key (auto)', async ({ page }) => {
  await page.goto('/tools/pem-public-key-extract/');
  await page.fill('#in-input', EC_PRIV);
  await page.selectOption('#in-key_type', 'auto');
  await expect(page.locator('#tool-output')).toContainText('-----BEGIN PUBLIC KEY-----', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText(EC_PUB_LINE, { timeout: 15000 });
});

test('pem-public-key-extract page derives EC public key via deep-link (explicit ec)', async ({
  page,
}) => {
  const qs = '?input=' + encodeURIComponent(EC_PRIV) + '&key_type=ec&der_format=hex';
  await page.goto('/tools/pem-public-key-extract/' + qs);
  await expect(page.locator('#tool-output')).toContainText(EC_PUB_LINE, { timeout: 15000 });
});

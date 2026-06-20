import { test, expect } from './fixtures';

// /tools/pem-to-jwk/ converts a PEM key to a JWK in-browser (pure wasm).
// The PEM is multi-line, so we drive it via the query-param deep-link (the input
// field is single-line) — the page auto-runs and feeds the raw value to wasm.
const EC_P256_PUB = `-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEoEin9VVEjidtQQwGmHavCvk+JREm
N7Z6PDsddFz9sdvsHE94DbN2kX4HCyhNnWNbA54OUiESeOMGsDexFXrF1A==
-----END PUBLIC KEY-----`;

test('pem-to-jwk page converts an EC P-256 public key', async ({ page }) => {
  await page.goto('/tools/pem-to-jwk/?input=' + encodeURIComponent(EC_P256_PUB));
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"kty": "EC"', { timeout: 15000 });
  await expect(out).toContainText('"crv": "P-256"');
  await expect(out).toContainText('"x":');
});

test('pem-to-jwk page reports a clear error for non-PEM input', async ({ page }) => {
  await page.goto('/tools/pem-to-jwk/?input=' + encodeURIComponent('not a key'));
  await expect(page.locator('#tool-output')).toContainText('PEM', { timeout: 15000 });
});

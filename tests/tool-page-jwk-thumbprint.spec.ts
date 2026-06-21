import { test, expect } from './fixtures';

const RFC_RSA =
  '{"kty":"RSA","n":"0vx7agoebGcQSuuPiLJXZptN9nndrQmbXEps2aiAFbWhM78LhWx4cbbfAAtVT86zwu1RK7aPFFxuhDR1L6tSoc_BJECPebWKRXjBZCiFV4n3oknjhMstn64tZ_2W-5JsGY4Hc5n9yBXArwl93lqt7_RN5w6Cf0h4QyQ5v-65YGjQR0_FDW2QvzqY368QQMicAtaSqzs8KJZgnYb9c7d0zgdAZHzu6qMQvRL5hajrn1n91CbOpbISD08qNLyrdkt-bFTWhAI4vMQFh6WeZu0fM4lFd2NcRwr3XPksINHaQ-G_xBniIqbw0Ls1jF44-csFCur-kEgU8awapJzKnqDKgw","e":"AQAB","alg":"RS256","kid":"2011-04-29"}';

// /tools/jwk-thumbprint/ computes RFC 7638 thumbprints in-browser (pure wasm).
test('matches the RFC 7638 RSA example thumbprint', async ({ page }) => {
  await page.goto('/tools/jwk-thumbprint/');
  await page.fill('#in-jwk', RFC_RSA);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('NzbLsXh8uDCcd-6MNwXF4W_7noWXFZAfHkxZsRGC9Xs', { timeout: 15000 });
  await expect(out).toContainText('kty: RSA');
});

test('reports an unsupported key type', async ({ page }) => {
  await page.goto('/tools/jwk-thumbprint/');
  await page.fill('#in-jwk', '{"kty":"XYZ"}');
  await expect(page.locator('#tool-output')).toContainText('unsupported key type', { timeout: 15000 });
});

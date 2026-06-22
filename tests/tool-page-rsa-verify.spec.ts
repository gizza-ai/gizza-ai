import { test, expect } from './fixtures';

// /tools/rsa-verify/ verifies an RSA signature in-browser (pure wasm).
// message + signature + public_key are multiline <textarea>; scheme + hash are <select>.
// PUB is the public key matching the throwaway RSA-2048 key in rsa-sign's spec;
// SIG is an openssl-produced PKCS#1 v1.5 / SHA-256 signature over "hello rsa".
const PUB = `-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAvJsEE3lVdO8+KqM3T+rT
Ka+jkjV7EY8pchmr5FWUL0+uq28lhB2TJ59gW8FTseK+df6bQ5bWZ0YAEH7RovHp
+W9geUgSMStEtavHghlvGWBsMt8cYJdjNIJ0qXZmaBrcnQoFlDB6Vp3KWvspAxVp
e39VBbCM9yCQFcrvqb7y+wyI8btaw6TU9fwBHqrxhyIXvzPg6tWoEpAv/iw+ev5W
xoMblsTidjXW56zklfH18Y/cv5r/rgrLdHuISEePOADGkqXcCee4AETQT+wgS4d2
En5zsQCIrRxKkuNCwhO7DC51/L5g8PKrFH2orupeW/Ptk7xtbEP/gBdvjItstN/u
wQIDAQAB
-----END PUBLIC KEY-----`;
const SIG = 'ALcAOl17JoN7rM7dZtapRR2WFJOT6+PyorGbZoat61+Y5Z1TjEYjsaaGmpEYsXixKzpdPXu5NkzDzKVzuhBuJDAlVdTxit5shv9PHZeG/HDUnL0yFfDFUDuT1KylX93cCrWBc4zxIICO23QbHWcZxZR8zit49cyoQuAadHEXodDiBb9mS/AYLldnIMZhkNg7k99z3u8vqFdEd6ptqvFJsoaYeW0JGtPJ2z3p4C7NYOoiwf2IVbjyKw+MkbImarRLuQryA+7S3oCszlQ7sTCeOuyD55pDNR7UOTSViB5DwUB+Ka0HM/6babunxgCS233xr5TOah2b0rEpZarSv4KGmA==';

test('rsa-verify page reports a valid signature', async ({ page }) => {
  await page.goto('/tools/rsa-verify/');
  await page.fill('#in-message', 'hello rsa');
  await page.fill('#in-signature', SIG);
  await page.fill('#in-public_key', PUB);
  await page.selectOption('#in-scheme', 'pkcs1v15');
  await page.selectOption('#in-hash', 'sha256');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('VALID', { timeout: 15000 });
  await expect(out).not.toContainText('INVALID');
});

test('rsa-verify page reports an invalid signature for a tampered message', async ({ page }) => {
  await page.goto('/tools/rsa-verify/');
  await page.fill('#in-message', 'goodbye rsa');
  await page.fill('#in-signature', SIG);
  await page.fill('#in-public_key', PUB);
  await page.selectOption('#in-scheme', 'pkcs1v15');
  await page.selectOption('#in-hash', 'sha256');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID', { timeout: 15000 });
});

test('rsa-verify page errors clearly on a bad key', async ({ page }) => {
  await page.goto('/tools/rsa-verify/');
  await page.fill('#in-message', 'hi');
  await page.fill('#in-signature', SIG);
  await page.fill('#in-public_key', 'not a key');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('public key', { timeout: 15000 });
});

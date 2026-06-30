import { test, expect } from './fixtures';

// /tools/rsa-encrypt/ encrypts a short message to an RSA public key in-browser (pure wasm).
// message + public_key are multiline <textarea>; padding + hash are enumv -> <select>.
// Test key is a throwaway RSA-2048 public key generated for this test.
const PUB = `-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEApDKXIdt5cNlICw7sgmqD
Z8WDttM7cLUq/z7zG8i1I0V9XWsGJoXRxYgatApPBwNTc8Nf5HBhxzMZAJCAAsje
AjpUcsymhmKramEUeVdpLjfXGhj+WYK8F5lgCNEK2mCCWKfElTx/PSeod8AvoPAU
ScjoDoF1+7/0TdMzYPlM7k3pzZTStETDsgnXlSVSJoX9pO58QhYcV2Jju67Z/UAz
sVzQMRIz5o2RDWcLlR4R95Ysc7FDjqiblqkjkmhJlT+QKGyMWs2+6y15voXcqWQ3
2hSBYAw//EWrDFOaRfqk3+z5t/Nsc2X9c2ArZH9YQYBLMsuh66UI3cBD89Snm9lb
qQIDAQAB
-----END PUBLIC KEY-----`;

test('rsa-encrypt page produces base64 ciphertext (oaep sha256)', async ({ page }) => {
  await page.goto('/tools/rsa-encrypt/');
  await page.fill('#in-message', 'hello rsa');
  await page.fill('#in-public_key', PUB);
  await page.selectOption('#in-padding', 'oaep');
  await page.selectOption('#in-hash', 'sha256');
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const txt = (await out.textContent())!.trim();
  // RSA-2048 ciphertext is 256 bytes -> 344 base64 chars.
  expect(txt).toMatch(/^[A-Za-z0-9+/]+=*$/);
  expect(txt.length).toBeGreaterThan(300);
});

test('rsa-encrypt page errors clearly on a bad key', async ({ page }) => {
  await page.goto('/tools/rsa-encrypt/');
  await page.fill('#in-message', 'hi');
  await page.fill('#in-public_key', 'not a key');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('public key', { timeout: 15000 });
});

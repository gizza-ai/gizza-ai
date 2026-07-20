import { test, expect } from './fixtures';

// RFC 6979 / secp256k1 known-answer vector: private key = 1, "Satoshi Nakamoto", SHA-256.
const KEY1 = '0000000000000000000000000000000000000000000000000000000000000001';
const PUB_C = '0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798';
const PUB_U =
  '0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8';
const KAT_SIG =
  '934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d82442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5';
const KAT_SIG_B64 =
  'k0seoQpLPBdX4rDAF9C2FDzjyafmpKSYYNemqyEO49gkQs6dK5FgZBCAFHg+kj7Da0l0Pi/6HESW8BpRKq/Z5Q==';
const KAT_DER =
  '3045022100934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d802202442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5';
// sha256("Satoshi Nakamoto") — the hash=none prehashed path.
const KAT_DIGEST = 'a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e';
const PKCS8_PEM =
  '-----BEGIN PRIVATE KEY-----\nMIGEAgEAMBAGByqGSM49AgEGBSuBBAAKBG0wawIBAQQgAAAAAAAAAAAAAAAAAAAA\nAAAAAAAAAAAAAAAAAAAAAAGhRANCAAR5vmZ++dy7rFWgYpXOhwsHApv82y3OKNlZ\n8oFbFvgXmEg62ncmo8RlXaT7/A4RCKj9F7RIpoVUGZxH0I/7ENS4\n-----END PRIVATE KEY-----';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

async function fillSign(page, message: string, key: string) {
  await page.selectOption('#in-operation', 'sign');
  await page.fill('#in-message', message);
  await page.fill('#in-key', key);
}

test('ecdsa-secp256k1 page generates a keypair on the default operation', async ({ page }) => {
  await page.goto('/tools/ecdsa-secp256k1/');
  await page.selectOption('#in-operation', 'generate');
  await expect(page.locator('#tool-output')).toContainText('operation: generate', { timeout: 15000 });
  const text = await outText(page);
  expect(text).toMatch(/private key \(hex\): [0-9a-f]{64}\n/);
  expect(text).toMatch(/public key \(compressed hex\): 0[23][0-9a-f]{64}\n/);
  expect(text).toMatch(/public key \(uncompressed hex\): 04[0-9a-f]{128}\n/);
  expect(text).toContain('-----BEGIN PRIVATE KEY-----');
  expect(text).toContain('-----BEGIN PUBLIC KEY-----');
});

test('ecdsa-secp256k1 page signs the RFC 6979 vector exactly', async ({ page }) => {
  await page.goto('/tools/ecdsa-secp256k1/');
  await fillSign(page, 'Satoshi Nakamoto', KEY1);
  await expect(page.locator('#tool-output')).toContainText(`signature (compact hex): ${KAT_SIG}`, {
    timeout: 15000,
  });
  const text = await outText(page);
  expect(text).toContain('hash: sha256');
  expect(text).toContain(`digest (hex): ${KAT_DIGEST}`);
  expect(text).toContain(`signature (compact base64): ${KAT_SIG_B64}`);
  expect(text).toContain(`signature (DER hex): ${KAT_DER}`);
  expect(text).toContain('r: 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8');
  expect(text).toContain('s: 2442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5');
  expect(text).toContain('recovery id: 1 (v = 28)');
  expect(text).toContain(`public key (compressed hex): ${PUB_C}`);
});

test('ecdsa-secp256k1 page verifies compact and DER signature forms', async ({ page }) => {
  await page.goto('/tools/ecdsa-secp256k1/');
  await page.selectOption('#in-operation', 'verify');
  await page.fill('#in-message', 'Satoshi Nakamoto');
  await page.fill('#in-key', PUB_C);
  await page.fill('#in-signature', KAT_SIG);
  await expect(page.locator('#tool-output')).toContainText('valid: true', { timeout: 15000 });
  expect(await outText(page)).toContain('signature form: compact');
  expect(await outText(page)).toContain('✓ signature is valid');
  // DER form + uncompressed public key.
  await page.fill('#in-key', PUB_U);
  await page.fill('#in-signature', KAT_DER);
  await expect(page.locator('#tool-output')).toContainText('signature form: der', { timeout: 15000 });
  expect(await outText(page)).toContain('valid: true');
  // Base64 compact signature.
  await page.fill('#in-signature', KAT_SIG_B64);
  await expect(page.locator('#tool-output')).toContainText('signature form: compact', { timeout: 15000 });
  expect(await outText(page)).toContain('valid: true');
});

test('ecdsa-secp256k1 page reports a wrong message as valid false', async ({ page }) => {
  await page.goto('/tools/ecdsa-secp256k1/');
  await page.selectOption('#in-operation', 'verify');
  await page.fill('#in-message', 'Satoshi Nakamot0');
  await page.fill('#in-key', PUB_C);
  await page.fill('#in-signature', KAT_SIG);
  await expect(page.locator('#tool-output')).toContainText('valid: false', { timeout: 15000 });
  expect(await outText(page)).toContain('✗ signature does NOT match');
});

test('ecdsa-secp256k1 page signs with every hash choice', async ({ page }) => {
  await page.goto('/tools/ecdsa-secp256k1/');
  // keccak256 (Ethereum) — deterministic, matches the CLI output exactly.
  await fillSign(page, 'hello ethereum', KEY1);
  await page.selectOption('#in-hash', 'keccak256');
  await expect(page.locator('#tool-output')).toContainText(
    'signature (compact hex): ba1429bd935a4ddc97b01238e2a981d5cb0c8f9c90ee992a95f5c6de3da8def554cf8ce511019fd036bde96ed265372f6ef91ba29407f4620d81526258240e72',
    { timeout: 15000 },
  );
  // sha384 + sha512.
  await page.fill('#in-message', 'rt');
  await page.selectOption('#in-hash', 'sha384');
  await expect(page.locator('#tool-output')).toContainText(
    'signature (compact hex): 231eaed29883347c49a0d6304f9a7241a35c550b6e2bf8ab2efd958b75fa59e702c682de9e99de7b1735d704cafe77beed0dbe3e29c87ab91b328aab22841a9f',
    { timeout: 15000 },
  );
  await page.selectOption('#in-hash', 'sha512');
  await expect(page.locator('#tool-output')).toContainText(
    'signature (compact hex): aa4d557c4a1efd0373639c128928b97dd3745e9e77b84e9d5e761f21f3d9bb2066a6f03e6239478109bb04200d654688d19f33d4b520a07acaef896c59fc0c4d',
    { timeout: 15000 },
  );
});

test('ecdsa-secp256k1 page signs a prehashed digest with hash none and 0x key', async ({ page }) => {
  await page.goto('/tools/ecdsa-secp256k1/');
  await fillSign(page, KAT_DIGEST, `0x${KEY1}`);
  await page.selectOption('#in-message_encoding', 'hex');
  await page.selectOption('#in-hash', 'none');
  // Same signature as hashing "Satoshi Nakamoto" with sha256 — the digest is the KAT digest.
  await expect(page.locator('#tool-output')).toContainText(`signature (compact hex): ${KAT_SIG}`, {
    timeout: 15000,
  });
  // Boundary: 31 and 33 bytes are rejected with an actionable error.
  await page.fill('#in-message', KAT_DIGEST.slice(0, 62));
  await expect(page.locator('#tool-output')).toContainText('32-byte digest', { timeout: 15000 });
  await page.fill('#in-message', `${KAT_DIGEST}00`);
  await expect(page.locator('#tool-output')).toContainText('32-byte digest', { timeout: 15000 });
});

test('ecdsa-secp256k1 page accepts a PEM private key', async ({ page }) => {
  await page.goto('/tools/ecdsa-secp256k1/');
  await fillSign(page, 'Satoshi Nakamoto', PKCS8_PEM);
  await expect(page.locator('#tool-output')).toContainText(`signature (compact hex): ${KAT_SIG}`, {
    timeout: 15000,
  });
});

test('ecdsa-secp256k1 page errors clearly when signing without a key', async ({ page }) => {
  await page.goto('/tools/ecdsa-secp256k1/');
  await page.selectOption('#in-operation', 'sign');
  await page.fill('#in-message', 'hello');
  await expect(page.locator('#tool-output')).toContainText('needs a private key', { timeout: 15000 });
});

test('ecdsa-secp256k1 example chip verifies the KAT signature', async ({ page }) => {
  await page.goto('/tools/ecdsa-secp256k1/');
  await page.locator('button.tool-example-chip', { hasText: 'Verify a signature' }).click();
  await expect(page.locator('#tool-output')).toContainText('valid: true', { timeout: 15000 });
});

test('ecdsa-secp256k1 deep-link signs a base64 message', async ({ page }) => {
  await page.goto(
    `/tools/ecdsa-secp256k1/?operation=sign&message=aGk%3D&message_encoding=base64&key=${KEY1}`,
  );
  // base64 "aGk=" = "hi" — deterministic signature, matches the CLI exactly.
  await expect(page.locator('#tool-output')).toContainText(
    'signature (compact hex): 831bf6fb51b475abcbbaf5cf34c92cfd816556c724fffcfad1a732aa2aa290c744d9ad4677774943c795db0b42f53b5b094ffaf8dafc34bde62ac90ec9184317',
    { timeout: 15000 },
  );
});

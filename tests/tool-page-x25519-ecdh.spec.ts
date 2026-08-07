import { test, expect } from './fixtures';

// /tools/x25519-ecdh/ performs an X25519 (Curve25519) ECDH key agreement in-browser
// (pure wasm). private_key/peer_public_key are multiline <textarea>s; kdf/encoding are
// <select>s; kdf_salt/kdf_info are text inputs; kdf_length is a number input and
// include_pem is a checkbox.
//
// Keys and the shared secret below are the RFC 7748 §6.1 test vector.
const ALICE_PRIV = '77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a';
const ALICE_PUB = '8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a';
const BOB_PUB = 'de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f';
const SHARED = '4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742';

async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

test('x25519-ecdh page derives the RFC 7748 shared secret exactly', async ({ page }) => {
  await page.goto('/tools/x25519-ecdh/');
  await page.fill('#in-private_key', ALICE_PRIV);
  await page.fill('#in-peer_public_key', BOB_PUB);
  await page.selectOption('#in-kdf', 'none');
  await page.selectOption('#in-encoding', 'hex');
  await expect(page.locator('#tool-output')).toContainText(`Shared secret      ${SHARED}`, {
    timeout: 15000,
  });
  // Whole report, verbatim — same text the CLI prints.
  expect((await outText(page)).trim()).toBe(
    [
      'X25519 ECDH · shared secret derived (hex)',
      '',
      `Your private key   ${ALICE_PRIV}`,
      `Your public key    ${ALICE_PUB}`,
      `Peer public key    ${BOB_PUB}`,
      '',
      `Shared secret      ${SHARED}`,
      '',
      'The raw shared secret is not a uniformly random key. Set kdf = hkdf-sha256 to expand it into one you can use directly.',
    ].join('\n'),
  );
});

test('x25519-ecdh page deep-links an HKDF-SHA256 session key via query params', async ({ page }) => {
  await page.goto(
    `/tools/x25519-ecdh/?private_key=${ALICE_PRIV}&peer_public_key=${BOB_PUB}` +
      '&kdf=hkdf-sha256&kdf_salt=handshake%20salt&kdf_info=app%20v1%20chat%20key' +
      '&kdf_length=32&encoding=hex&include_pem=false',
  );
  await expect(page.locator('#in-private_key')).toHaveValue(ALICE_PRIV);
  await expect(page.locator('#in-peer_public_key')).toHaveValue(BOB_PUB);
  await expect(page.locator('#in-kdf_salt')).toHaveValue('handshake salt');
  const out = page.locator('#tool-output');
  await expect(out).toContainText(
    'Derived key        eb806d7e8576658943c8b2e586d4fd2c46a558a9e4e4c3d13b134e82af943d4c',
    { timeout: 15000 },
  );
  expect(await outText(page)).toContain(
    'Derived with hkdf-sha256 · 32 bytes · salt: "handshake salt" · info: "app v1 chat key"',
  );
});

test('x25519-ecdh page emits hex, base64 and base64url keys', async ({ page }) => {
  await page.goto('/tools/x25519-ecdh/');
  await page.fill('#in-private_key', ALICE_PRIV);
  await page.fill('#in-peer_public_key', BOB_PUB);
  await page.selectOption('#in-kdf', 'none');
  const out = page.locator('#tool-output');

  await page.selectOption('#in-encoding', 'hex');
  await expect(out).toContainText(`Shared secret      ${SHARED}`, { timeout: 15000 });

  await page.selectOption('#in-encoding', 'base64');
  await expect(out).toContainText(
    'Shared secret      Sl2dW6TOLeFyjjv0gDUPJeB+IclH0Z4zdvCbPB4WF0I=',
    { timeout: 15000 },
  );
  expect(await outText(page)).toContain(
    'Your public key    hSDwCYkwp1R0i33ctD73Wg2/Og0mOBr066SpjqqbTmo=',
  );

  await page.selectOption('#in-encoding', 'base64url');
  await expect(out).toContainText('Shared secret      Sl2dW6TOLeFyjjv0gDUPJeB-IclH0Z4zdvCbPB4WF0I', {
    timeout: 15000,
  });
  const url = await outText(page);
  expect(url).toContain('X25519 ECDH · shared secret derived (base64url)');
  expect(url).toContain('Your public key    hSDwCYkwp1R0i33ctD73Wg2_Og0mOBr066SpjqqbTmo');
});

test('x25519-ecdh page covers every kdf choice', async ({ page }) => {
  await page.goto('/tools/x25519-ecdh/');
  await page.fill('#in-private_key', ALICE_PRIV);
  await page.fill('#in-peer_public_key', BOB_PUB);
  await page.selectOption('#in-encoding', 'hex');
  await page.fill('#in-kdf_salt', 'handshake salt');
  await page.fill('#in-kdf_info', 'app v1 chat key');
  const out = page.locator('#tool-output');

  // none — the raw RFC 7748 output, flagged as not directly usable.
  await page.selectOption('#in-kdf', 'none');
  await expect(out).toContainText(
    'The raw shared secret is not a uniformly random key.',
    { timeout: 15000 },
  );
  expect(await outText(page)).not.toContain('Derived key');

  // hkdf-sha256, 32 bytes.
  await page.selectOption('#in-kdf', 'hkdf-sha256');
  await page.fill('#in-kdf_length', '32');
  await expect(out).toContainText(
    'Derived key        eb806d7e8576658943c8b2e586d4fd2c46a558a9e4e4c3d13b134e82af943d4c',
    { timeout: 15000 },
  );

  // hkdf-sha512, 64 bytes.
  await page.selectOption('#in-kdf', 'hkdf-sha512');
  await page.fill('#in-kdf_length', '64');
  await expect(out).toContainText(
    'Derived key        b97b1541e768917de513e840c6df8ed5c004ddfc144d1916fa8f0dfb8bedebb8' +
      'cf9ea59e523766f4df264c97a3a1a77fc8ff2cfada56d6f73b3449dcb05ae447',
    { timeout: 15000 },
  );
  expect(await outText(page)).toContain(
    'Derived with hkdf-sha512 · 64 bytes · salt: "handshake salt" · info: "app v1 chat key"',
  );

  // sha256 — a plain hash of the secret; salt/info are ignored.
  await page.selectOption('#in-kdf', 'sha256');
  await expect(out).toContainText(
    'Derived key        dead45a1d43d6902aa9240b43c0d75a0b5fc750660590d6d45461cbfc4010684',
    { timeout: 15000 },
  );
  expect(await outText(page)).toContain('Derived with sha256 · 32 bytes');
});

test('x25519-ecdh page exports PKCS#8 / SPKI PEM blocks', async ({ page }) => {
  await page.goto('/tools/x25519-ecdh/');
  await page.fill('#in-private_key', ALICE_PRIV);
  await page.fill('#in-peer_public_key', BOB_PUB);
  await page.selectOption('#in-kdf', 'none');
  await page.selectOption('#in-encoding', 'hex');
  await page.check('#in-include_pem');
  await expect(page.locator('#tool-output')).toContainText('Your private key (PKCS#8 PEM)', {
    timeout: 15000,
  });
  const text = await outText(page);
  expect(text).toContain(
    '-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VuBCIEIHcHbQpzGKV9PBbBclGyZkXfTC+H68CZKrF3+6UduSwq\n-----END PRIVATE KEY-----',
  );
  expect(text).toContain('Your public key (SPKI PEM)');
  expect(text).toContain(
    '-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VuAyEAhSDwCYkwp1R0i33ctD73Wg2/Og0mOBr066SpjqqbTmo=\n-----END PUBLIC KEY-----',
  );
  expect(text).toContain('Peer public key (SPKI PEM)');
  expect(text).toContain(
    '-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VuAyEA3p7bfXt9wbTTW2HC7OQ1Nz+DQ8hbeGdNrfx+FG+IK08=\n-----END PUBLIC KEY-----',
  );
});

test('x25519-ecdh page accepts kdf_length 8160 and rejects 8161', async ({ page }) => {
  await page.goto('/tools/x25519-ecdh/');
  await page.fill('#in-private_key', ALICE_PRIV);
  await page.fill('#in-peer_public_key', BOB_PUB);
  await page.selectOption('#in-kdf', 'hkdf-sha256');
  await page.selectOption('#in-encoding', 'hex');

  // 8160 = the HKDF 255 × hash-length ceiling for SHA-256.
  await page.fill('#in-kdf_length', '8160');
  await expect(page.locator('#tool-output')).toContainText(
    'Derived with hkdf-sha256 · 8160 bytes',
    { timeout: 15000 },
  );
  const derived = (await outText(page))
    .split('\n')
    .find((line) => line.startsWith('Derived key'))!;
  expect(derived.replace('Derived key', '').trim()).toHaveLength(8160 * 2);

  // One byte past the ceiling is an error, not a truncated key.
  await page.fill('#in-kdf_length', '8161');
  await expect(page.locator('#tool-output')).toContainText(
    'kdf_length must be between 1 and 8160 bytes; got 8161',
    { timeout: 15000 },
  );
});

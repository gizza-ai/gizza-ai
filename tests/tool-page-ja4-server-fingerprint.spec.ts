import { test, expect } from './fixtures';

// /tools/ja4-server-fingerprint/ computes the JA4S TLS server fingerprint from a
// ServerHello (hex) in-browser (pure wasm).

// A hand-built TLS record carrying a ServerHello:
//   legacy_version 0x0303, cipher c02b, no session id,
//   extensions: supported_versions(002b)->0304, key_share(0033), alpn(0010)->h2.
const SERVER_HELLO =
  '16030300630200005f0303' +
  '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f' +
  '00c02b00' +
  '0037' +
  '002b0002030400330024001d00200000000000000000000000000000000000000000000000000000000000000000001000050003026832';

test('ja4-server-fingerprint computes the JA4S string', async ({ page }) => {
  await page.goto('/tools/ja4-server-fingerprint/');
  await page.fill('#in-server_hello', SERVER_HELLO);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('t1303h2_c02b_19fd10492780', { timeout: 15000 });
  await expect(out).toContainText('TLS 1.3 (0x0304)');
  await expect(out).toContainText('"transport": "TCP"');
  await expect(out).toContainText('h2');
});

test('ja4-server-fingerprint uses the q prefix for QUIC', async ({ page }) => {
  await page.goto('/tools/ja4-server-fingerprint/');
  await page.fill('#in-server_hello', SERVER_HELLO);
  await page.check('#in-quic');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('q1303h2_c02b_19fd10492780', { timeout: 15000 });
  await expect(out).toContainText('"transport": "QUIC"');
});

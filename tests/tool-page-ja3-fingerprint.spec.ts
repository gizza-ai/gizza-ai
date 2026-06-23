import { test, expect } from './fixtures';

// /tools/ja3-fingerprint/ computes the JA3 TLS client fingerprint from a
// ClientHello (hex) in-browser (pure wasm).

// A hand-built TLS 1.2 record carrying a ClientHello:
//   version 0x0303, ciphers GREASE(0a0a) 1301 c02b, comp 00,
//   SNI "gizza.ai", supported_groups GREASE(0a0a) 001d 0017,
//   ec_point_formats 00, GREASE extension 1a1a.
const CLIENT_HELLO =
  '160301005a010000560303' +
  'abababababababababababababababababababababababababababababababab' +
  '0000060a0a1301c02b' +
  '010000270000000d000b00000867697a7a612e6169' +
  '000a000800060a0a001d0017' +
  '000b00020100' +
  '1a1a0000';

test('ja3-fingerprint computes the JA3 string and MD5', async ({ page }) => {
  await page.goto('/tools/ja3-fingerprint/');
  await page.fill('#in-client_hello', CLIENT_HELLO);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('771,4865-49195,0-10-11,29-23,0', { timeout: 15000 });
  await expect(out).toContainText('3e916670429427a5a33c947802616cdc');
  await expect(out).toContainText('ja3n');
  await expect(out).toContainText('TLS 1.2 (0x0303)');
  await expect(out).toContainText('gizza.ai');
});

test('ja3-fingerprint accepts a 0x prefix and separators', async ({ page }) => {
  await page.goto('/tools/ja3-fingerprint/');
  await page.fill('#in-client_hello', '0x' + CLIENT_HELLO);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('771,4865-49195,0-10-11,29-23,0', { timeout: 15000 });
});

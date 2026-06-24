import { test, expect } from './fixtures';

// /tools/parse-tls-record/ decodes TLS record-layer bytes from hex in-browser (pure wasm).

// A hand-built TLS 1.2 handshake record carrying a ClientHello with SNI
// (example.com), ALPN (h2, http/1.1) and supported_versions (TLS 1.3, 1.2).
// Record: 16 03 01 | len | handshake 01 | len | ClientHello...
const CLIENT_HELLO =
  '16030100600100005c0303' +
  'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' + // 32-byte random
  '00' + // session id len 0
  '00041301c02f' + // cipher suites: TLS_AES_128_GCM_SHA256, 0xc02f
  '0100' + // compression: null
  '002f' + // extensions length = 0x2f (47)
  // server_name "example.com"
  '00000010000e00000b6578616d706c652e636f6d' +
  // ALPN h2, http/1.1
  '0010000e000c02683208687474702f312e31' +
  // supported_versions TLS 1.3, TLS 1.2
  '002b00050403040303';

test('parse-tls-record decodes a ClientHello: cipher suites, SNI, ALPN', async ({ page }) => {
  await page.goto('/tools/parse-tls-record/');
  await page.fill('#in-record', CLIENT_HELLO);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Content Type:', { timeout: 15000 });
  await expect(out).toContainText('handshake');
  await expect(out).toContainText('client_hello');
  await expect(out).toContainText('TLS_AES_128_GCM_SHA256');
  await expect(out).toContainText('example.com');
  await expect(out).toContainText('h2');
});

test('parse-tls-record decodes multiple concatenated records', async ({ page }) => {
  await page.goto('/tools/parse-tls-record/');
  // change_cipher_spec (14 03 03 00 01 01) + alert warning close_notify (15 03 03 00 02 01 00)
  await page.fill('#in-record', '14030300010115030300020100');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Record 1 of 2', { timeout: 15000 });
  await expect(out).toContainText('change_cipher_spec');
  await expect(out).toContainText('Record 2 of 2');
  await expect(out).toContainText('close_notify');
});

test('parse-tls-record decodes an alert record', async ({ page }) => {
  await page.goto('/tools/parse-tls-record/');
  // type 0x15 alert, version 0x0303, length 2, fatal handshake_failure (02 28)
  await page.fill('#in-record', '15 03 03 00 02 02 28');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('alert', { timeout: 15000 });
  await expect(out).toContainText('fatal');
  await expect(out).toContainText('handshake_failure');
});

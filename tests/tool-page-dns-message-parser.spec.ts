import { test, expect } from './fixtures';

// /tools/dns-message-parser/ decodes a DNS wire-format message from hex or
// base64url in-browser (pure wasm).

// A hand-built DNS response: id 0x1234, flags 0x8180 (response, RD, RA, NOERROR),
// qd=1 an=1. Question example.com A IN; answer uses a 0xC00C compression pointer
// back to "example.com", A IN ttl 60, rdata 93.184.216.34.
const RESPONSE_HEX =
  '12348180000100010000000007' +
  '6578616d706c6503636f6d00' + // example com
  '00010001' + // QTYPE A, QCLASS IN
  'c00c00010001' + // pointer, A, IN
  '0000003c0004' + // ttl 60, rdlen 4
  '5db8d822'; // 93.184.216.34

test('dns-message-parser decodes a response: header, question, A record', async ({ page }) => {
  await page.goto('/tools/dns-message-parser/');
  await page.fill('#in-message', RESPONSE_HEX);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Header:', { timeout: 15000 });
  await expect(out).toContainText('QR:       response');
  await expect(out).toContainText('NOERROR');
  await expect(out).toContainText('example.com');
  await expect(out).toContainText('93.184.216.34');
});

test('dns-message-parser accepts base64url (DNS-over-HTTPS form)', async ({ page }) => {
  await page.goto('/tools/dns-message-parser/');
  // base64url of a minimal query for example.com A: id 0, flags 0x0100 (RD),
  // qd 1. Q: example.com A IN.
  // bytes: 0000 0100 0001 0000 0000 0000 07 example 03 com 00 0001 0001
  await page.fill(
    '#in-message',
    'AAABAAABAAAAAAAAB2V4YW1wbGUDY29tAAABAAE',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('QR:       query', { timeout: 15000 });
  await expect(out).toContainText('example.com');
  await expect(out).toContainText('RD');
});

test('dns-message-parser deep-links via the ?message= query param', async ({ page }) => {
  await page.goto('/tools/dns-message-parser/?message=' + RESPONSE_HEX);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('93.184.216.34', { timeout: 15000 });
});

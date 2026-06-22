import { test, expect } from './fixtures';

// /tools/strip-udp-header/ removes the fixed 8-byte UDP header from a raw UDP
// datagram (hex) and returns the encapsulated payload in-browser (pure wasm).
test('strip-udp-header returns the payload and decodes the header fields', async ({ page }) => {
  await page.goto('/tools/strip-udp-header/');
  // 8-byte header (src 53, dst 41394, len 0x0c, csum 0x1234) + payload deadbeef.
  await page.fill('#in-datagram', '0035 a1b2 000c 1234 deadbeef');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Payload (hex):', { timeout: 15000 });
  await expect(out).toContainText('deadbeef');
  await expect(out).toContainText('Source port:');
  await expect(out).toContainText('53');
  await expect(out).toContainText('41394');
  await expect(out).toContainText('8 bytes');
});

test('strip-udp-header honors the Length field and drops trailing padding', async ({ page }) => {
  await page.goto('/tools/strip-udp-header/');
  // UDP length 0x0c = 12 but 4 trailing padding bytes (ffffffff) must be dropped.
  await page.fill('#in-datagram', '0035 a1b2 000c 1234 deadbeef ffffffff');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('deadbeef', { timeout: 15000 });
  await expect(out).not.toContainText('ffffffff');
  await expect(out).toContainText('Payload length:    4 bytes');
});

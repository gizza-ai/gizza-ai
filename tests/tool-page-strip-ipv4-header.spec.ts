import { test, expect } from './fixtures';

// /tools/strip-ipv4-header/ removes the IPv4 header from a raw packet (hex) and
// returns the encapsulated payload in-browser (pure wasm).
test('strip-ipv4-header returns the UDP payload and drops the 20-byte header', async ({ page }) => {
  await page.goto('/tools/strip-ipv4-header/');
  // 20-byte header (proto UDP, total len 0x1c) + 8-byte payload 0035003500080000.
  await page.fill('#in-packet', '4500 001c 1c46 4000 4011 0000 c0a8 0068 c0a8 0001 0035 0035 0008 0000');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Payload (hex):', { timeout: 15000 });
  await expect(out).toContainText('0035003500080000');
  await expect(out).toContainText('UDP');
  await expect(out).toContainText('20 bytes');
});

test('strip-ipv4-header honors IHL and strips IP options', async ({ page }) => {
  await page.goto('/tools/strip-ipv4-header/');
  // IHL 6 (24-byte header) with a 4-byte option (07040500), proto TCP, then
  // payload deadbeef. The option bytes must not appear in the payload.
  await page.fill('#in-packet', '4600 001c 1c46 4000 4006 0000 c0a8 0068 c0a8 0001 0704 0500 dead beef');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('deadbeef', { timeout: 15000 });
  await expect(out).toContainText('TCP');
  await expect(out).toContainText('24 bytes');
});

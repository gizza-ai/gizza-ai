import { test, expect } from './fixtures';

// /tools/parse-ipv4-header/ decodes a raw IPv4 packet header from hex in-browser (pure wasm).
test('parse-ipv4-header decodes version, protocol, addresses, checksum', async ({ page }) => {
  await page.goto('/tools/parse-ipv4-header/');
  // 45 00 00 3c 1c 46 40 00 40 06 9c bc c0 a8 00 68 c0 a8 00 01
  // version 4, IHL 5, DF set, TTL 64, proto TCP, valid checksum, 192.168.0.104 → 192.168.0.1
  await page.fill('#in-header', '4500 003c 1c46 4000 4006 9cbc c0a8 0068 c0a8 0001');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Version:', { timeout: 15000 });
  await expect(out).toContainText('(IPv4)');
  await expect(out).toContainText('TCP');
  await expect(out).toContainText('192.168.0.104');
  await expect(out).toContainText('192.168.0.1');
  await expect(out).toContainText('DF');
  await expect(out).toContainText('valid');
});

test('parse-ipv4-header decodes DSCP, fragmentation and a UDP protocol', async ({ page }) => {
  await page.goto('/tools/parse-ipv4-header/');
  // ToS 0xb8 → DSCP 46 (EF); flags/frag MF + offset 42; proto UDP(17)
  await page.fill('#in-header', '45b8 05dc 1c46 202a 4011 0000 0a00 0001 0a00 0002');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('EF (Expedited Forwarding)', { timeout: 15000 });
  await expect(out).toContainText('MF');
  await expect(out).toContainText('UDP');
});

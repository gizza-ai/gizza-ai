import { test, expect } from './fixtures';

// /tools/parse-ethernet-frame/ decodes an Ethernet frame from hex in-browser (pure wasm).
test('parse-ethernet-frame decodes MAC, VLAN, EtherType, payload', async ({ page }) => {
  await page.goto('/tools/parse-ethernet-frame/');
  // dst aabbccddeeff, src 112233445566, 802.1Q VID=10 PCP=1, EtherType 0x0800 (IPv4), payload deadbeef
  await page.fill('#in-frame', 'aabbccddeeff112233445566 8100 200a 0800 deadbeef');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Ethernet II', { timeout: 15000 });
  await expect(out).toContainText('aa:bb:cc:dd:ee:ff');
  await expect(out).toContainText('11:22:33:44:55:66');
  await expect(out).toContainText('VLAN tag #1');
  await expect(out).toContainText('VID=10');
  await expect(out).toContainText('0x0800');
  await expect(out).toContainText('IPv4');
  await expect(out).toContainText('deadbeef');
});

test('parse-ethernet-frame decodes a broadcast ARP frame', async ({ page }) => {
  await page.goto('/tools/parse-ethernet-frame/');
  await page.fill(
    '#in-frame',
    'ffffffffffff0011223344550806000108000604000100112233445500000000',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('ff:ff:ff:ff:ff:ff', { timeout: 15000 });
  await expect(out).toContainText('broadcast');
  await expect(out).toContainText('ARP');
});

import { test, expect } from './fixtures';

// /tools/parse-udp-header/ decodes a raw UDP datagram header from hex in-browser (pure wasm).
test('parse-udp-header decodes ports, service, length and checksum', async ({ page }) => {
  await page.goto('/tools/parse-udp-header/');
  // c3d2 0035 0028 1b6e → src 50130, dst 53 (DNS), length 40 (32 payload), cksum 0x1b6e
  await page.fill('#in-header', 'c3d2 0035 0028 1b6e');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Source Port:', { timeout: 15000 });
  await expect(out).toContainText('50130');
  await expect(out).toContainText('Destination Port:');
  await expect(out).toContainText('53 (DNS)');
  await expect(out).toContainText('40 bytes');
  await expect(out).toContainText('0x1b6e');
});

test('parse-udp-header notes a disabled checksum', async ({ page }) => {
  await page.goto('/tools/parse-udp-header/');
  // checksum 0x0000 → disabled
  await page.fill('#in-header', '0035 c3d2 001c 0000');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('disabled', { timeout: 15000 });
});

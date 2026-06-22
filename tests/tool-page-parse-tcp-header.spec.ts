import { test, expect } from './fixtures';

// /tools/parse-tcp-header/ decodes a raw TCP segment header from hex in-browser (pure wasm).
test('parse-tcp-header decodes ports, flags, sequence and window', async ({ page }) => {
  await page.goto('/tools/parse-tcp-header/');
  // c213 0050 4f8e 6b1a 0000 0000 5002 ffff fe34 0000
  // src port 49683, dst 80, seq 0x4f8e6b1a, data offset 5, SYN, window 65535, cksum 0xfe34
  await page.fill('#in-header', 'c213 0050 4f8e 6b1a 0000 0000 5002 ffff fe34 0000');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Source Port:', { timeout: 15000 });
  await expect(out).toContainText('49683');
  await expect(out).toContainText('Destination Port:');
  await expect(out).toContainText('80');
  await expect(out).toContainText('SYN');
  await expect(out).toContainText('65535');
  await expect(out).toContainText('0xfe34');
});

test('parse-tcp-header decodes options (MSS, Window Scale) on a longer header', async ({ page }) => {
  await page.goto('/tools/parse-tcp-header/');
  // data offset 7 → options MSS=1460, NOP, Window Scale=7
  await page.fill('#in-header', 'c213 0050 4f8e 6b1a 0000 0000 7002 ffff fe34 0000 0204 05b4 0103 0307');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Maximum Segment Size (MSS)', { timeout: 15000 });
  await expect(out).toContainText('1460');
  await expect(out).toContainText('Window Scale');
});

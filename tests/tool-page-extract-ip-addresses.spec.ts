import { test, expect } from './fixtures';

// /tools/extract-ip-addresses/ finds + validates IPs in-browser (pure wasm).
test('extract-ip-addresses finds IPv4 and IPv6, rejects junk', async ({ page }) => {
  await page.goto('/tools/extract-ip-addresses/');
  await page.fill(
    '#in-text',
    'client 192.168.0.1:443 -> 10.0.0.5, dup 192.168.0.1, v6 2001:0db8::1 and [fe80::a]:22, time 12:34:56',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('4 unique IP address(es)', { timeout: 15000 });
  await expect(out).toContainText('192.168.0.1');
  await expect(out).toContainText('10.0.0.5');
  await expect(out).toContainText('2001:db8::1'); // canonicalized
  await expect(out).toContainText('fe80::a');
});

test('extract-ip-addresses reports when none present', async ({ page }) => {
  await page.goto('/tools/extract-ip-addresses/');
  await page.fill('#in-text', 'no addresses in this line at all');
  await expect(page.locator('#tool-output')).toContainText('No IP', { timeout: 15000 });
});

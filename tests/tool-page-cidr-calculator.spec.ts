import { test, expect } from './fixtures';

test('cidr-calculator page: IPv4 subnet report (text)', async ({ page }) => {
  await page.goto('/tools/cidr-calculator/');

  // Default format is 'text' (first <select> option). A non-aligned /26 host
  // normalizes to its network address.
  await page.fill('#in-input', '192.168.1.130/26');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Network address:    192.168.1.128', {
    timeout: 15000,
  });
  await expect(out).toContainText('Broadcast address:  192.168.1.191');
  await expect(out).toContainText('Netmask:            255.255.255.192 (/26)');
  await expect(out).toContainText('Wildcard mask:      0.0.0.63');
  await expect(out).toContainText('Usable host range:  192.168.1.129 - 192.168.1.190');
  await expect(out).toContainText('Usable hosts:       62');
  await expect(out).toContainText('Scope:              Private');
});

test('cidr-calculator page: JSON output and IPv6', async ({ page }) => {
  await page.goto('/tools/cidr-calculator/');

  // JSON format for an IPv4 /30.
  await page.fill('#in-input', '10.0.0.0/30');
  await page.selectOption('#in-format', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"family": "IPv4"', { timeout: 15000 });
  await expect(out).toContainText('"broadcast": "10.0.0.3"');
  await expect(out).toContainText('"usable_hosts": 2');

  // IPv6 in text mode — total address count is exact for a /64.
  await page.selectOption('#in-format', 'text');
  await page.fill('#in-input', '2001:db8::/64');
  await expect(out).toContainText('Address family:     IPv6', { timeout: 15000 });
  await expect(out).toContainText('Total addresses:    18446744073709551616');
});

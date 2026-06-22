import { test, expect } from './fixtures';

test('ip-range-expand page: CIDR and range list', async ({ page }) => {
  await page.goto('/tools/ip-range-expand/');

  // Default output is 'list' (first <select> option).
  await page.fill('#in-input', '10.0.0.0/30');
  await expect(page.locator('#tool-output')).toHaveText(
    '10.0.0.0\n10.0.0.1\n10.0.0.2\n10.0.0.3',
    { timeout: 15000 },
  );

  // A start-end IPv4 range.
  await page.fill('#in-input', '192.168.1.10-192.168.1.13');
  await expect(page.locator('#tool-output')).toHaveText(
    '192.168.1.10\n192.168.1.11\n192.168.1.12\n192.168.1.13',
    { timeout: 15000 },
  );
});

test('ip-range-expand page: count output and IPv6', async ({ page }) => {
  await page.goto('/tools/ip-range-expand/');

  // Count mode for an IPv4 /24.
  await page.fill('#in-input', '192.168.1.0/24');
  await page.selectOption('#in-output', 'count');
  await expect(page.locator('#tool-output')).toHaveText('256', {
    timeout: 15000,
  });

  // Count is exact even for a huge IPv6 /64.
  await page.fill('#in-input', '2001:db8::/64');
  await expect(page.locator('#tool-output')).toHaveText(
    '18446744073709551616',
    { timeout: 15000 },
  );

  // IPv6 CIDR list.
  await page.selectOption('#in-output', 'list');
  await page.fill('#in-input', '2001:db8::/126');
  await expect(page.locator('#tool-output')).toHaveText(
    '2001:db8::\n2001:db8::1\n2001:db8::2\n2001:db8::3',
    { timeout: 15000 },
  );
});

import { test, expect } from './fixtures';

test('ip-range-to-cidr page: unaligned range to minimal CIDR list', async ({ page }) => {
  await page.goto('/tools/ip-range-to-cidr/');

  // Default output is 'list' (first <select> option). The classic example.
  await page.fill('#in-input', '10.0.0.5-10.0.0.20');
  await expect(page.locator('#tool-output')).toHaveText(
    '10.0.0.5/32\n10.0.0.6/31\n10.0.0.8/29\n10.0.0.16/30\n10.0.0.20/32',
    { timeout: 15000 },
  );

  // An aligned range collapses to a single CIDR.
  await page.fill('#in-input', '192.168.1.0-192.168.1.255');
  await expect(page.locator('#tool-output')).toHaveText('192.168.1.0/24', {
    timeout: 15000,
  });
});

test('ip-range-to-cidr page: count output and IPv6', async ({ page }) => {
  await page.goto('/tools/ip-range-to-cidr/');

  // Count mode: how many CIDR blocks does the range need.
  await page.fill('#in-input', '10.0.0.5-10.0.0.20');
  await page.selectOption('#in-output', 'count');
  await expect(page.locator('#tool-output')).toHaveText('5', {
    timeout: 15000,
  });

  // IPv6 range, list mode.
  await page.selectOption('#in-output', 'list');
  await page.fill('#in-input', '2001:db8::1-2001:db8::5');
  await expect(page.locator('#tool-output')).toHaveText(
    '2001:db8::1/128\n2001:db8::2/127\n2001:db8::4/127',
    { timeout: 15000 },
  );
});

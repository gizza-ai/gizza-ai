import { test, expect } from './fixtures';

// /tools/extract-mac-addresses/ finds + normalizes MACs in-browser (pure wasm).
test('extract-mac-addresses finds + normalizes + dedups MACs', async ({ page }) => {
  await page.goto('/tools/extract-mac-addresses/');
  await page.fill(
    '#in-text',
    'nic 00:1A:2B:3C:4D:5E, same as 00-1a-2b-3c-4d-5e, router 001a.2b3c.4d6f, bare AABBCCDDEEFF, hash d41d8cd98f00b204e9800998ecf8427e',
  );
  const out = page.locator('#tool-output');
  // Default format is colon; the dup (00:..:5e) counts once → 3 unique.
  await expect(out).toContainText('3 unique MAC address(es)', { timeout: 15000 });
  await expect(out).toContainText('00:1a:2b:3c:4d:5e');
  await expect(out).toContainText('00:1a:2b:3c:4d:6f'); // cisco-form router, normalized
  await expect(out).toContainText('aa:bb:cc:dd:ee:ff'); // bare hex, normalized
});

test('extract-mac-addresses honors the cisco output format', async ({ page }) => {
  await page.goto('/tools/extract-mac-addresses/');
  await page.fill('#in-text', 'device aa:bb:cc:dd:ee:ff online');
  await page.selectOption('#in-format', 'cisco');
  await expect(page.locator('#tool-output')).toContainText('aabb.ccdd.eeff', { timeout: 15000 });
});

test('extract-mac-addresses reports when none present', async ({ page }) => {
  await page.goto('/tools/extract-mac-addresses/');
  await page.fill('#in-text', 'no mac addresses in this line at all');
  await expect(page.locator('#tool-output')).toContainText('No MAC', { timeout: 15000 });
});

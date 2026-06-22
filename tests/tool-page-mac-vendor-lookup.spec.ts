import { test, expect } from './fixtures';

test('mac-vendor-lookup resolves a known vendor', async ({ page }) => {
  await page.goto('/tools/mac-vendor-lookup/');

  await page.fill('#in-mac', '28:6F:B9:01:23:45');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Vendor: Nokia', { timeout: 15000 });
  await expect(out).toContainText('OUI:    28:6F:B9');
  await expect(out).toContainText('unicast address');
});

test('mac-vendor-lookup accepts the Cisco dotted form', async ({ page }) => {
  await page.goto('/tools/mac-vendor-lookup/');
  await page.fill('#in-mac', '0000.0c12.3456');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Vendor: Cisco', { timeout: 15000 });
  await expect(out).toContainText('OUI:    00:00:0C');
});

test('mac-vendor-lookup reports an unassigned OUI and a bad input', async ({
  page,
}) => {
  await page.goto('/tools/mac-vendor-lookup/');
  await page.fill('#in-mac', 'FF:FF:FF:FF:FF:FF');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('not in IEEE registry', { timeout: 15000 });

  await page.fill('#in-mac', 'zz');
  await expect(out).toContainText('Error:', { timeout: 15000 });
});

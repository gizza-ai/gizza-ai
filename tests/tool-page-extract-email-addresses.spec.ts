import { test, expect } from './fixtures';

// /tools/extract-email-addresses/ extracts + dedupes emails in-browser (pure wasm).
test('extract-email-addresses lists unique addresses', async ({ page }) => {
  await page.goto('/tools/extract-email-addresses/');
  await page.fill(
    '#in-text',
    'Email alice@corp.com or bob@corp.com. Also Carol@OTHER.io and alice@corp.com again.',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('3 unique address(es)', { timeout: 15000 });
  await expect(out).toContainText('alice@corp.com');
  await expect(out).toContainText('carol@other.io');
});

test('extract-email-addresses groups by domain when checked', async ({ page }) => {
  await page.goto('/tools/extract-email-addresses/');
  await page.fill('#in-text', 'alice@corp.com bob@corp.com carol@other.io');
  await page.check('#in-group_by_domain');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('@corp.com (2)', { timeout: 15000 });
  await expect(out).toContainText('@other.io (1)');
});

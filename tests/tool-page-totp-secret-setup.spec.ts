import { test, expect } from './fixtures';

// /tools/totp-secret-setup/ generates a fresh local TOTP secret and otpauth URI.
test('totp-secret-setup page generates a default setup', async ({ page }) => {
  await page.goto('/tools/totp-secret-setup/');
  await page.fill('#in-issuer', 'Example');
  await page.fill('#in-account', 'alice@example.com');
  await expect(page.locator('#tool-output')).toContainText('Secret:', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Bits: 160');
  await expect(page.locator('#tool-output')).toContainText('Algorithm: SHA1');
  await expect(page.locator('#tool-output')).toContainText('URI: otpauth://totp/Example:alice%40example.com?', { timeout: 15000 });
});

test('totp-secret-setup page honors deep-linked advanced values', async ({ page }) => {
  await page.goto('/tools/totp-secret-setup/?issuer=Example&account=bob%40example.com&secret_bytes=10&digits=8&period=60&algorithm=sha256');
  await expect(page.locator('#tool-output')).toContainText('Secret:', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Bits: 80');
  await expect(page.locator('#tool-output')).toContainText('Algorithm: SHA256');
  await expect(page.locator('#tool-output')).toContainText('Digits: 8');
  await expect(page.locator('#tool-output')).toContainText('Period: 60 seconds');
  await expect(page.locator('#tool-output')).toContainText('bob%40example.com');
});

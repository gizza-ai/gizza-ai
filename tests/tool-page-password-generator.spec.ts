import { test, expect } from './fixtures';

// /tools/password-generator/ generates a random password/passphrase in-browser.
test('password-generator page generates a password', async ({ page }) => {
  await page.goto('/tools/password-generator/');
  await page.fill('#in-length', '24');
  // default mode (empty select -> password). Output should show entropy bits.
  await expect(page.locator('#tool-output')).toContainText('bits of entropy', { timeout: 15000 });
});

test('password-generator page makes a passphrase via deep-link', async ({ page }) => {
  await page.goto('/tools/password-generator/?mode=passphrase&words=4&separator=-');
  // 4 dash-separated words → at least 3 dashes in the output line.
  await expect(page.locator('#tool-output')).toContainText('-', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('bits of entropy');
});

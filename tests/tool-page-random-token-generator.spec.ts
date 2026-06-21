import { test, expect } from './fixtures';

// /tools/random-token-generator/ generates cryptographically random tokens in-browser.
test('random-token-generator page generates a hex token', async ({ page }) => {
  await page.goto('/tools/random-token-generator/');
  await page.fill('#in-length', '32');
  // default charset (empty -> hex). Output should report entropy bits.
  await expect(page.locator('#tool-output')).toContainText('bits of entropy', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('128.0');
});

test('random-token-generator page makes a batch via deep-link', async ({ page }) => {
  await page.goto('/tools/random-token-generator/?length=24&count=3&charset=base64url');
  // 3 tokens → output spans multiple lines plus the entropy summary line.
  await expect(page.locator('#tool-output')).toContainText('bits of entropy', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('64-character alphabet');
});

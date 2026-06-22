import { test, expect } from './fixtures';

test('bcrypt-hash page hashes a password', async ({ page }) => {
  await page.goto('/tools/bcrypt-hash/');
  await page.fill('#in-password', 'correct horse');
  await page.selectOption('#in-mode', 'hash');
  await page.fill('#in-cost', '6');
  await expect(page.locator('#tool-output')).toHaveText(/^\$2b\$06\$.{53}$/, { timeout: 20000 });
});

test('bcrypt-hash page verifies a known hash', async ({ page }) => {
  await page.goto('/tools/bcrypt-hash/');
  await page.fill('#in-password', 'password');
  await page.selectOption('#in-mode', 'verify');
  await page.fill('#in-hash', '$2b$10$odaUG9d1NgVaLONfFhi3hOg4b7Xje3Mw.OF/nyPS0EBKUUbkpDDIW');
  await expect(page.locator('#tool-output')).toContainText('match', { timeout: 20000 });
});

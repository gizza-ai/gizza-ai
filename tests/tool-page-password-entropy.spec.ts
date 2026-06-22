import { test, expect } from './fixtures';

// /tools/password-entropy/ estimates strength in-browser (pure wasm).
test('password-entropy page flags a common password', async ({ page }) => {
  await page.goto('/tools/password-entropy/');
  await page.fill('#in-password', 'password');
  await expect(page.locator('#tool-output')).toContainText('common password', { timeout: 15000 });
});

test('password-entropy page rates a strong password via deep-link', async ({ page }) => {
  await page.goto('/tools/password-entropy/?password=' + encodeURIComponent('8#kQ!v2pL@9zR4nW'));
  await expect(page.locator('#tool-output')).toContainText('bits', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('trong');
});

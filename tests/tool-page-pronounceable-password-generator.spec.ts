import { test, expect } from './fixtures';

// /tools/pronounceable-password-generator/ builds a speakable password in-browser.
test('pronounceable-password-generator page generates a password', async ({ page }) => {
  await page.goto('/tools/pronounceable-password-generator/');
  await page.fill('#in-length', '12');
  await expect(page.locator('#tool-output')).toContainText('bits of entropy', { timeout: 15000 });
});

test('pronounceable-password-generator page honors deep-link params', async ({ page }) => {
  // 16 lowercase letters, no digits, no symbols → a pure a–z pronounceable string.
  await page.goto('/tools/pronounceable-password-generator/?length=16&capitalize=false&digits=0&symbols=0');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('bits of entropy', { timeout: 15000 });
  const text = await out.innerText();
  const pw = text.split('\n')[0].trim();
  expect(pw).toMatch(/^[a-z]{16}$/);
});

import { test, expect } from './fixtures';

test('hash-identifier page identifies bcrypt', async ({ page }) => {
  await page.goto('/tools/hash-identifier/');
  await page.fill(
    '#in-input',
    '$2b$12$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW',
  );
  await expect(page.locator('#tool-output')).toContainText('bcrypt', {
    timeout: 15000,
  });
});

test('hash-identifier page lists MD5 and NTLM for 32-hex', async ({ page }) => {
  await page.goto('/tools/hash-identifier/');
  await page.fill('#in-input', '5f4dcc3b5aa765d61d8327deb882cf99');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('MD5', { timeout: 15000 });
  await expect(out).toContainText('NTLM', { timeout: 15000 });
});

import { test, expect } from './fixtures';

test('email-validator page accepts a valid address', async ({ page }) => {
  await page.goto('/tools/email-validator/');

  await page.fill('#in-input', 'user@example.com');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid: yes', { timeout: 15000 });
  await expect(out).toContainText('Domain: example.com');
  await expect(out).toContainText('Errors: none');
  await expect(out).toContainText('Suggestion: none');
});

test('email-validator flags a misspelled provider with a suggestion', async ({
  page,
}) => {
  await page.goto('/tools/email-validator/');
  await page.fill('#in-input', 'user@gmial.com');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Suggestion: user@gmail.com', {
    timeout: 15000,
  });
  await expect(out).toContainText('Valid: yes');
});

test('email-validator reports errors for an invalid address', async ({
  page,
}) => {
  await page.goto('/tools/email-validator/');
  await page.fill('#in-input', 'not-an-email');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid: no', { timeout: 15000 });
  await expect(out).toContainText("missing '@'");
});

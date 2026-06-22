import { test, expect } from './fixtures';

test('email-normalizer page canonicalizes a Gmail address', async ({ page }) => {
  await page.goto('/tools/email-normalizer/');

  await page.fill('#in-email', 'John.Doe+newsletter@googlemail.com');
  const out = page.locator('#tool-output');
  // Gmail: dots stripped, +tag dropped, googlemail.com folded to gmail.com.
  await expect(out).toContainText('Canonical: johndoe@gmail.com', {
    timeout: 15000,
  });
  await expect(out).toContainText('Provider: Gmail');
  await expect(out).toContainText('Sub-address tag: +newsletter (removed)');
  await expect(out).toContainText('Dots removed: yes');
});

test('email-normalizer keeps dots for non-Gmail providers', async ({ page }) => {
  await page.goto('/tools/email-normalizer/');
  await page.fill('#in-email', 'First.Last+promo@Outlook.com');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Canonical: first.last@outlook.com', {
    timeout: 15000,
  });
  await expect(out).toContainText('Dots removed: no');
});

test('email-normalizer preserves local case when checkbox unticked', async ({
  page,
}) => {
  await page.goto('/tools/email-normalizer/');
  await page.fill('#in-email', 'Jane.Roe@Example.COM');
  // Default-checked box lowercases the local part; uncheck to preserve case.
  await page.uncheck('#in-lowercase_local');
  await expect(page.locator('#tool-output')).toContainText(
    'Canonical: Jane.Roe@example.com',
    { timeout: 15000 },
  );
});

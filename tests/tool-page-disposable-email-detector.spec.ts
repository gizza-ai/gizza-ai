import { test, expect } from './fixtures';

test('disposable-email-detector flags a known throwaway provider', async ({
  page,
}) => {
  await page.goto('/tools/disposable-email-detector/');

  await page.fill('#in-input', 'someone@mailinator.com');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Disposable: yes', { timeout: 15000 });
  await expect(out).toContainText('Domain: mailinator.com');
  await expect(out).toContainText('Matched: mailinator.com');
});

test('disposable-email-detector passes a real provider', async ({ page }) => {
  await page.goto('/tools/disposable-email-detector/');
  await page.fill('#in-input', 'jane@gmail.com');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Disposable: no', { timeout: 15000 });
  await expect(out).toContainText('Domain: gmail.com');
});

test('disposable-email-detector accepts a bare domain', async ({ page }) => {
  await page.goto('/tools/disposable-email-detector/');
  await page.fill('#in-input', 'yopmail.com');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Disposable: yes', { timeout: 15000 });
  await expect(out).toContainText('Domain: yopmail.com');
});

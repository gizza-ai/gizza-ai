import { test, expect } from './fixtures';

test('base64url-converter converts standard base64 to canonical url-safe output', async ({ page }) => {
  await page.goto('/tools/base64url-converter/');
  await page.fill('#in-text', 'c3ViamVjdHM/X2Q9MQ==');
  await page.selectOption('#in-direction', 'to-url');
  await expect(page.locator('#tool-output')).toHaveText('c3ViamVjdHM_X2Q9MQ', { timeout: 15000 });
});

test('base64url-converter honours a deep link for url-safe to standard padded output', async ({ page }) => {
  const qs = '?text=c3ViamVjdHM_X2Q9MQ&direction=to-standard&padding=auto';
  await page.goto('/tools/base64url-converter/' + qs);
  await expect(page.locator('#in-direction')).toHaveValue('to-standard', { timeout: 15000 });
  await expect(page.locator('#in-padding')).toHaveValue('auto');
  await expect(page.locator('#tool-output')).toHaveText('c3ViamVjdHM/X2Q9MQ==', { timeout: 15000 });
});

test('base64url-converter validates input and keeps padding when requested', async ({ page }) => {
  await page.goto('/tools/base64url-converter/');
  await page.fill('#in-text', '+/+/');
  await page.selectOption('#in-direction', 'to-url');
  await page.selectOption('#in-padding', 'keep');
  await page.check('#in-validate');
  await expect(page.locator('#tool-output')).toHaveText('-_-_', { timeout: 15000 });
});

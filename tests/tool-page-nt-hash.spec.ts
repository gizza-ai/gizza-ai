import { test, expect } from './fixtures';

test('nt-hash page computes standard password vector', async ({ page }) => {
  await page.goto('/tools/nt-hash/');
  await page.fill('#in-password', 'password');
  await expect(page.locator('#tool-output')).toHaveText('8846f7eaee8fb117ad06bdd830b7586c', {
    timeout: 15000,
  });
});

test('nt-hash page supports uppercase hex', async ({ page }) => {
  await page.goto('/tools/nt-hash/');
  await page.fill('#in-password', 'password');
  await page.check('#in-uppercase');
  await expect(page.locator('#tool-output')).toHaveText('8846F7EAEE8FB117AD06BDD830B7586C', {
    timeout: 15000,
  });
});

test('nt-hash page supports base64 output', async ({ page }) => {
  await page.goto('/tools/nt-hash/');
  await page.fill('#in-password', 'password');
  await page.selectOption('#in-output_format', 'base64');
  await expect(page.locator('#tool-output')).toHaveText('iEb36u6PsRetBr3YMLdYbA==', {
    timeout: 15000,
  });
});

test('nt-hash query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/nt-hash/?password=' + encodeURIComponent('123456'));
  await expect(page.locator('#in-password')).toHaveValue('123456', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('32ed87bdb5fdc5e9cba88547376818d4', {
    timeout: 15000,
  });
});

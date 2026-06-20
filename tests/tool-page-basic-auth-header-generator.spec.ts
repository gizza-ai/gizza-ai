import { test, expect } from './fixtures';
test('basic-auth-header-generator page builds the value', async ({ page }) => {
  await page.goto('/tools/basic-auth-header-generator/');
  await page.fill('#in-username', 'aladdin');
  await page.fill('#in-password', 'opensesame');
  await expect(page.locator('#tool-output')).toHaveText('Basic YWxhZGRpbjpvcGVuc2VzYW1l', { timeout: 15000 });
});
test('basic-auth query-param deep-link + full header', async ({ page }) => {
  await page.goto('/tools/basic-auth-header-generator/?username=u&password=p&full_header=true');
  await expect(page.locator('#in-username')).toHaveValue('u', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toHaveText('Authorization: Basic dTpw', { timeout: 15000 });
});

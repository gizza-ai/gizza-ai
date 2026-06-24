import { test, expect } from './fixtures';

test('otpauth-uri page builds a totp provisioning URI', async ({ page }) => {
  await page.goto('/tools/otpauth-uri/');
  await page.fill('#in-issuer', 'GitHub');
  await page.fill('#in-account', 'alice@example.com');
  await page.fill('#in-secret', 'JBSWY3DPEHPK3PXP');
  await expect(page.locator('#tool-output')).toHaveText(
    'otpauth://totp/GitHub:alice%40example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub&algorithm=SHA1&digits=6&period=30',
    { timeout: 15000 }
  );
});

test('otpauth-uri page supports hotp with a counter', async ({ page }) => {
  await page.goto('/tools/otpauth-uri/');
  await page.selectOption('#in-type', 'hotp');
  await page.fill('#in-issuer', 'ACME');
  await page.fill('#in-account', 'bob');
  await page.fill('#in-secret', 'JBSWY3DPEHPK3PXP');
  await page.fill('#in-counter', '5');
  await expect(page.locator('#tool-output')).toContainText('otpauth://hotp/ACME:bob?', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('&counter=5', { timeout: 15000 });
});

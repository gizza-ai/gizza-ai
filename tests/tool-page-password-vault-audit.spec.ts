import { test, expect } from './fixtures';

const vaultCsv = [
  'name,username,password,url,totp',
  'Email,ada@example.com,P@ssw0rd,http://mail.example.com,',
  'Bank,ada,CorrectHorseBatteryStaple!,https://bank.example.com,otpauth://totp/Bank',
  'Shop,ada,CorrectHorseBatteryStaple!,https://shop.example.com,',
].join('\n');

test('password-vault-audit page reports reuse and weak passwords', async ({ page }) => {
  await page.goto('/tools/password-vault-audit/');
  await page.fill('#in-data', vaultCsv);
  await page.selectOption('#in-format', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Vault audit — 3 entries read as csv', { timeout: 15000 });
  await expect(out).toContainText('Findings: 3 errors, 2 warnings');
  await expect(out).toContainText('[reused-password] Bank, Shop');
  await expect(out).toContainText('[common-password] Email');
  await expect(out).toContainText('[weak-password] Email');
  await expect(out).toContainText('[insecure-url] Email');
  await expect(out).not.toContainText('CorrectHorseBatteryStaple!');
});

test('password-vault-audit deep-links strict JSON options', async ({ page }) => {
  await page.goto(
    '/tools/password-vault-audit/?' +
      new URLSearchParams({
        data: vaultCsv,
        format: 'csv',
        min_length: '16',
        min_score: '60',
        check_missing_2fa: 'true',
        mask_passwords: 'true',
        output: 'json',
      }).toString()
  );

  await expect(page.locator('#in-data')).toHaveValue(vaultCsv, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#in-min_length')).toHaveValue('16');
  await expect(page.locator('#in-min_score')).toHaveValue('60');
  await expect(page.locator('#in-check_missing_2fa')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"format": "csv"', { timeout: 15000 });
  await expect(out).toContainText('"rule": "reused-password"');
  await expect(out).toContainText('"rule": "missing-2fa"');
  await expect(out).not.toContainText('P@ssw0rd');
});

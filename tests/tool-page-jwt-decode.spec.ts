import { test, expect } from './fixtures';

function b64url(s: string): string {
  return Buffer.from(s, 'utf-8')
    .toString('base64')
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

test('jwt-decode page decodes a valid token and displays details', async ({ page }) => {
  await page.goto('/tools/jwt-decode/');

  // Far future exp to ensure it is valid
  const header = '{"alg":"HS256","typ":"JWT"}';
  const payload = '{"sub":"1234567890","name":"John Doe","iat":1516239022,"exp":4102444800}';
  const token = `${b64url(header)}.${b64url(payload)}.signature-segment`;

  await page.fill('#in-token', token);

  const out = page.locator('#tool-output');
  
  // Status banner should show active token
  await expect(out.locator('.jwt-status-banner')).toContainText('Active Token', { timeout: 15000 });

  // Highlighting box should show colorized text
  await expect(out.locator('.jwt-color-header')).toContainText(b64url(header));
  await expect(out.locator('.jwt-color-payload')).toContainText(b64url(payload));
  await expect(out.locator('.jwt-color-signature')).toContainText('signature-segment');

  // JSON code blocks should contain parsed properties
  await expect(out.locator('.jwt-decoded-header-card pre')).toContainText('"alg": "HS256"');
  await expect(out.locator('.jwt-decoded-payload-card pre')).toContainText('"sub": "1234567890"');
  await expect(out.locator('.jwt-decoded-payload-card pre')).toContainText('"name": "John Doe"');

  // Claims Validation checks
  await expect(out.locator('.jwt-check-item.jwt-check-ok')).toHaveCount(3); // signature_present + exp + iat
});

test('jwt-decode page flags an expired token and handles leeway', async ({ page }) => {
  await page.goto('/tools/jwt-decode/');

  const header = '{"alg":"HS256"}';
  // exp in the past (epoch 1000)
  const payload = '{"sub":"123","exp":1000}';
  const token = `${b64url(header)}.${b64url(payload)}.sig`;

  await page.fill('#in-token', token);

  const out = page.locator('#tool-output');
  
  // Validation should be flagged (expired)
  await expect(out.locator('.jwt-status-banner')).toContainText('Validation Flagged', { timeout: 15000 });
  await expect(out.locator('.jwt-check-item.jwt-check-fail')).toContainText('exp');

  // Set clock-skew leeway to 2 billion seconds (larger than 2026 epoch) to verify leeway
  await page.fill('#in-leeway', '2000000000');
  
  // Now it should show as valid/active
  await expect(out.locator('.jwt-status-banner')).toContainText('Active Token');
  await expect(out.locator('.jwt-check-item.jwt-check-ok')).toHaveCount(2); // signature_present + exp
});

test('jwt-decode page shows error for malformed token', async ({ page }) => {
  await page.goto('/tools/jwt-decode/');
  await page.fill('#in-token', 'malformed.token.value.with.too.many.dots');

  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/);
  await expect(out).toContainText('Invalid JWT format');
});

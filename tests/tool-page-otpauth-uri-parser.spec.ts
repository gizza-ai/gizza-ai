import { test, expect } from './fixtures';

const TOTP_URI =
  'otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&algorithm=SHA1&digits=6&period=30';
const HOTP_URI = 'otpauth://hotp/ACME:bob@example.com?secret=K5XXE3DE&issuer=ACME&counter=5&digits=8';

async function outputJson(page) {
  const text = await page.locator('#tool-output').textContent({ timeout: 20000 });
  return JSON.parse(text ?? '');
}

test('otpauth-uri-parser page parses a TOTP URI into JSON fields', async ({ page }) => {
  await page.goto('/tools/otpauth-uri-parser/');
  await page.fill('#in-uri', TOTP_URI);

  const parsed = await outputJson(page);
  expect(parsed.type).toBe('totp');
  expect(parsed.issuer).toBe('ACME Co');
  expect(parsed.account).toBe('john.doe@email.com');
  expect(parsed.secret).toBe('HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ');
  expect(parsed.algorithm).toBe('SHA1');
  expect(parsed.digits).toBe(6);
  expect(parsed.period).toBe(30);
  expect(parsed.counter).toBeNull();
});

test('otpauth-uri-parser page supports text, table, and masked secrets', async ({ page }) => {
  await page.goto('/tools/otpauth-uri-parser/');
  await page.fill('#in-uri', HOTP_URI);
  await page.selectOption('#in-format', 'table');
  await expect(page.locator('#tool-output')).toContainText('counter', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('5');

  await page.selectOption('#in-format', 'text');
  await page.check('#in-mask_secret');
  const output = page.locator('#tool-output');
  await expect(output).toContainText('type:        hotp', { timeout: 15000 });
  await expect(output).toContainText('account:     bob@example.com');
  await expect(output).toContainText('secret:      ********');
  await expect(output).not.toContainText('K5XXE3DE');
});

test('otpauth-uri-parser query-param deep-link prefills controls and strict mode', async ({ page }) => {
  const uri = 'otpauth://totp/GitHub:alice?secret=JBSWY3DPEHPK3PXP&issuer=GitLab';
  await page.goto(
    '/tools/otpauth-uri-parser/?uri=' +
      encodeURIComponent(uri) +
      '&format=json&strict=true&mask_secret=true',
  );

  await expect(page.locator('#in-uri')).toHaveValue(uri, { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('json');
  await expect(page.locator('#in-strict')).toBeChecked();
  await expect(page.locator('#in-mask_secret')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveClass(/error/, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('issuer mismatch: the label says');
});

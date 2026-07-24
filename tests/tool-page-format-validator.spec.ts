import { test, expect } from './fixtures';

async function setValue(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-input').evaluate((el, v) => {
    (el as HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('format-validator auto-detects a valid email and shows all checks', async ({ page }) => {
  await page.goto('/tools/format-validator/');
  await setValue(page, 'user@example.com');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Detected:   email', { timeout: 15_000 });
  await expect(out).toContainText('email        valid');
  await expect(out).toContainText('url          invalid');
  await expect(out).toContainText("valid email; local 'user', domain 'example.com'");
});

test('format-validator deep-links URL JSON output', async ({ page }) => {
  const qs = new URLSearchParams({
    input: 'https://example.com/path?q=1',
    format: 'url',
    output: 'json',
  });
  await page.goto(`/tools/format-validator/?${qs.toString()}`);

  await expect(page.locator('#in-input')).toHaveValue('https://example.com/path?q=1');
  await expect(page.locator('#in-format')).toHaveValue('url');
  await expect(page.locator('#in-output')).toHaveValue('json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"format": "url"', { timeout: 15_000 });
  await expect(out).toContainText('"valid": true');
  await expect(out).toContainText("valid URL; scheme 'https', host 'example.com'");
});

test('format-validator covers every explicit format option', async ({ page }) => {
  await page.goto('/tools/format-validator/');

  await setValue(page, '192.168.1.1');
  await page.selectOption('#in-format', 'ipv4');
  await expect(page.locator('#tool-output')).toContainText('valid IPv4 address', { timeout: 15_000 });

  await setValue(page, '2001:db8::1');
  await page.selectOption('#in-format', 'ipv6');
  await expect(page.locator('#tool-output')).toContainText('valid IPv6 address', { timeout: 15_000 });

  await page.selectOption('#in-format', 'ip');
  await expect(page.locator('#tool-output')).toContainText('valid IP address (IPv6)', { timeout: 15_000 });

  await setValue(page, '+1 (415) 555-2671');
  await page.selectOption('#in-format', 'phone');
  await expect(page.locator('#tool-output')).toContainText('valid phone number', { timeout: 15_000 });

  await setValue(page, '4111 1111 1111 1111');
  await page.selectOption('#in-format', 'credit-card');
  await expect(page.locator('#tool-output')).toContainText('Luhn OK', { timeout: 15_000 });
});

test('format-validator reports validation errors and invalid values', async ({ page }) => {
  await page.goto('/tools/format-validator/');
  await setValue(page, 'not-an-email');
  await page.selectOption('#in-format', 'email');
  await expect(page.locator('#tool-output')).toContainText("missing '@'", { timeout: 15_000 });

  await setValue(page, '4111 1111 1111 1112');
  await page.selectOption('#in-format', 'credit-card');
  await expect(page.locator('#tool-output')).toContainText('fails the Luhn checksum', { timeout: 15_000 });

  await setValue(page, '');
  await expect(page.locator('#tool-output')).toContainText('input is empty', { timeout: 15_000 });
});

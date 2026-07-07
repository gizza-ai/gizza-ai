import { test, expect } from './fixtures';

// /tools/pii-tokenize/ replaces PII with deterministic, format-preserving tokens
// in-browser (pure wasm). Outputs are EXACT: the web (wasm32-unknown-unknown) and
// CLI (wasm32-wasip1) share the same HMAC-SHA256 core + built-in default key, so
// the page output equals the CLI `tokenized` field byte-for-byte. Fields:
// #in-text (multiline textarea), #in-secret (text), #in-preserve_email_domain
// (checkbox, default checked).

test('tokenizes email deterministically + linkably (default key, domain kept)', async ({ page }) => {
  await page.goto('/tools/pii-tokenize/');
  await page.fill('#in-text', 'a ada@example.com b ada@example.com c');
  // both occurrences map to the SAME token, domain preserved, original gone.
  await expect(page.locator('#tool-output')).toHaveText(
    'a tmm@example.com b tmm@example.com c',
    { timeout: 15000 },
  );
});

test('preserve_email_domain OFF pseudonymizes the whole address (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/pii-tokenize/');
  await page.fill('#in-text', 'ping ada@example.com now');
  await page.uncheck('#in-preserve_email_domain');
  await expect(page.locator('#tool-output')).toHaveText('ping tmm@fuocved.msq now', {
    timeout: 15000,
  });
});

test('a secret key changes the mapping', async ({ page }) => {
  await page.goto('/tools/pii-tokenize/');
  await page.fill('#in-text', 'ping ada@example.com now');
  await page.fill('#in-secret', 'key-a');
  await expect(page.locator('#tool-output')).toHaveText('ping jny@example.com now', {
    timeout: 15000,
  });
});

test('card stays Luhn-shaped, SSN + IPv4 keep format', async ({ page }) => {
  await page.goto('/tools/pii-tokenize/');
  await page.fill('#in-text', 'card 4111 1111 1111 1111 ssn 123-45-6789 host 192.168.0.1');
  await expect(page.locator('#tool-output')).toHaveText(
    'card 5240 0522 2407 0227 ssn 203-06-1223 host 139.11.241.147',
    { timeout: 15000 },
  );
});

test('phone keeps punctuation + length', async ({ page }) => {
  await page.goto('/tools/pii-tokenize/');
  await page.fill('#in-text', 'call (415) 555-0132 today');
  await expect(page.locator('#tool-output')).toHaveText('call (868) 818-0931 today', {
    timeout: 15000,
  });
});

test('IPv6 groups stay valid hex', async ({ page }) => {
  await page.goto('/tools/pii-tokenize/');
  await page.fill('#in-text', 'addr 2001:0db8:85a3:0000:0000:8a2e:0370:7334 x');
  await expect(page.locator('#tool-output')).toHaveText(
    'addr 4aa1:19e0:7f24:5fd6:c732:d325:a5ce:8435 x',
    { timeout: 15000 },
  );
});

test('deep-link prefills text and auto-runs', async ({ page }) => {
  await page.goto('/tools/pii-tokenize/?text=' + encodeURIComponent('ping ada@example.com now'));
  await expect(page.locator('#tool-output')).toHaveText('ping tmm@example.com now', {
    timeout: 15000,
  });
});

test('deep-link boolean param (preserve_email_domain=false) unchecks + reruns', async ({ page }) => {
  await page.goto(
    '/tools/pii-tokenize/?text=' +
      encodeURIComponent('ping ada@example.com now') +
      '&preserve_email_domain=false',
  );
  await expect(page.locator('#tool-output')).toHaveText('ping tmm@fuocved.msq now', {
    timeout: 15000,
  });
});

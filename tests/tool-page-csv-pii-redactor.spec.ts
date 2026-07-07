import { test, expect } from './fixtures';

// /tools/csv-pii-redactor/ masks / salted-hashes / redacts chosen CSV columns
// in-browser (pure wasm). mode is a <select>; header a checkbox (default on);
// keep_last / hash_length are sliders (canonical #in-<name> number box).

// Multi-line output: compare textContent exactly (toHaveText normalizes whitespace).
async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

test('mask mode (default) masks only the named column', async ({ page }) => {
  await page.goto('/tools/csv-pii-redactor/');
  await page.fill('#in-data', 'name,email\nAda,ada@example.com');
  await page.fill('#in-columns', 'email');
  // wait for the run to produce output
  await expect(page.locator('#tool-output')).toContainText('***', { timeout: 15000 });
  // ada@example.com is 15 chars -> 15 stars; name + header row unchanged.
  expect(await outText(page)).toBe('name,email\nAda,***************\n');
});

test('keep-last slider leaves the tail visible (mask mode)', async ({ page }) => {
  await page.goto('/tools/csv-pii-redactor/');
  await page.fill('#in-data', 'id,card\n1,4111111111111111');
  await page.fill('#in-columns', 'card');
  await page.fill('#in-keep_last', '4'); // non-default slider value
  await expect(page.locator('#tool-output')).toContainText('1111', { timeout: 15000 });
  expect(await outText(page)).toBe('id,card\n1,************1111\n');
});

test('hash mode is salted and deterministic (enum=hash)', async ({ page }) => {
  await page.goto('/tools/csv-pii-redactor/');
  await page.fill('#in-data', 'user\nada\nada');
  await page.fill('#in-columns', 'user');
  await page.selectOption('#in-mode', 'hash');
  await page.fill('#in-salt', 'team-2026');
  await expect(page.locator('#tool-output')).toContainText('c683217e', { timeout: 15000 });
  // header 'user' passes through; both 'ada' rows map to the SAME salted code.
  expect(await outText(page)).toBe('user\nc683217e\nc683217e\n');
});

test('redact mode + no-header + index addressing (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/csv-pii-redactor/');
  await page.fill('#in-data', 'Ada,123-45-6789');
  await page.fill('#in-columns', '2');
  await page.selectOption('#in-mode', 'redact');
  await page.uncheck('#in-header'); // header default is on; test the off-path
  await expect(page.locator('#tool-output')).toContainText('[REDACTED]', { timeout: 15000 });
  expect(await outText(page)).toBe('Ada,[REDACTED]\n');
});

test('deep-link pre-fills and auto-runs (redact via ?params)', async ({ page }) => {
  const data = encodeURIComponent('name,ssn\nAda,123-45-6789');
  await page.goto(`/tools/csv-pii-redactor/?data=${data}&columns=ssn&mode=redact&label=${encodeURIComponent('[PII]')}`);
  await expect(page.locator('#tool-output')).toContainText('[PII]', { timeout: 15000 });
  expect(await outText(page)).toBe('name,ssn\nAda,[PII]\n');
});

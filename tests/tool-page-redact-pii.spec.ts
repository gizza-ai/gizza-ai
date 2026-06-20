import { test, expect } from './fixtures';

// /tools/redact-pii/ masks PII in text in-browser (pure wasm). The text field is
// a multiline <textarea>; style is a plain field.
test('redact-pii page labels detected PII', async ({ page }) => {
  await page.goto('/tools/redact-pii/');
  await page.fill('#in-text', 'reach ada@example.com or 415-555-0132');
  await page.selectOption('#in-style', 'label');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('[EMAIL]', { timeout: 15000 });
  await expect(out).toContainText('[PHONE]');
});

test('redact-pii page masks via deep-link', async ({ page }) => {
  await page.goto(
    '/tools/redact-pii/?text=' + encodeURIComponent('ada@example.com') + '&style=mask',
  );
  await expect(page.locator('#tool-output')).toContainText('***', { timeout: 15000 });
});

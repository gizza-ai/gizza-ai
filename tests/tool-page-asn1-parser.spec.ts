import { test, expect } from './fixtures';

test('asn1-parser page renders a DER sequence tree', async ({ page }) => {
  await page.goto('/tools/asn1-parser/');
  await page.fill('#in-input', '30 06 02 01 01 01 01 FF');
  await page.selectOption('#in-format', 'tree');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('SEQUENCE (0x30) len=6 @0', { timeout: 15000 });
  await expect(out).toContainText('INTEGER (0x02) len=1 @2: 1 (0x01)');
  await expect(out).toContainText('BOOLEAN (0x01) len=1 @5: TRUE');
});

test('asn1-parser page decodes common OIDs', async ({ page }) => {
  await page.goto('/tools/asn1-parser/');
  await page.fill('#in-input', '06 09 2A 86 48 86 F7 0D 01 01 0B');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('OBJECT IDENTIFIER (0x06) len=9 @0', { timeout: 15000 });
  await expect(out).toContainText('1.2.840.113549.1.1.11 (sha256WithRSAEncryption)');
});

test('asn1-parser page supports JSON output', async ({ page }) => {
  await page.goto('/tools/asn1-parser/');
  await page.fill('#in-input', '0500');
  await page.selectOption('#in-format', 'json');
  const text = (await page.locator('#tool-output').textContent({ timeout: 15000 })) ?? '';
  const parsed = JSON.parse(text);
  expect(parsed.tag).toBe('NULL');
  expect(parsed.length).toBe(0);
});

test('asn1-parser page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/asn1-parser/?input=02012A&format=tree');
  await expect(page.locator('#in-input')).toHaveValue('02012A');
  await expect(page.locator('#in-format')).toHaveValue('tree');
  await expect(page.locator('#tool-output')).toHaveText('INTEGER (0x02) len=1 @0: 42 (0x2A)', { timeout: 15000 });
});

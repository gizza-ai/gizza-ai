import { test, expect } from './fixtures';

// /tools/mac-address-format/ reformats MAC addresses in-browser (pure wasm).
test('mac-address-format reformats one address to the default colon/lower style', async ({ page }) => {
  await page.goto('/tools/mac-address-format/');
  await page.fill('#in-input', 'AA-BB-CC-DD-EE-FF');
  // Default format is colon, default case is lower.
  await expect(page.locator('#tool-output')).toContainText('aa:bb:cc:dd:ee:ff', { timeout: 15000 });
});

test('mac-address-format honors the cisco format and upper case', async ({ page }) => {
  await page.goto('/tools/mac-address-format/');
  await page.fill('#in-input', '00:1a:2b:3c:4d:5e');
  await page.selectOption('#in-format', 'cisco');
  await page.selectOption('#in-case', 'upper');
  await expect(page.locator('#tool-output')).toContainText('001A.2B3C.4D5E', { timeout: 15000 });
});

test('mac-address-format reformats many, preserving order and duplicates', async ({ page }) => {
  await page.goto('/tools/mac-address-format/');
  await page.fill('#in-input', '11:22:33:44:55:66, aabbccddeeff\n11:22:33:44:55:66');
  await page.selectOption('#in-format', 'hyphen');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('11-22-33-44-55-66', { timeout: 15000 });
  await expect(out).toContainText('aa-bb-cc-dd-ee-ff');
});

test('mac-address-format flags an invalid address', async ({ page }) => {
  await page.goto('/tools/mac-address-format/');
  await page.fill('#in-input', '00:11:22:33:44');
  await expect(page.locator('#tool-output')).toContainText('not a MAC address', { timeout: 15000 });
});

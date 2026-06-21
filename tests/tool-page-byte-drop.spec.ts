import { test, expect } from './fixtures';

// /tools/byte-drop/ removes a byte range in-browser (pure wasm).

test('byte-drop removes a range from text', async ({ page }) => {
  await page.goto('/tools/byte-drop/');
  // Defaults: format=text, output=text.
  await page.fill('#in-input', 'Hello, World');
  await page.fill('#in-start', '5');
  await page.fill('#in-length', '4');
  await expect(page.locator('#tool-output')).toHaveText('Hellorld', {
    timeout: 15000,
  });
});

test('byte-drop negative start counts from the end', async ({ page }) => {
  await page.goto('/tools/byte-drop/');
  await page.fill('#in-input', 'abcdef');
  await page.fill('#in-start', '-2');
  await page.fill('#in-length', '2');
  await expect(page.locator('#tool-output')).toHaveText('abcd', {
    timeout: 15000,
  });
});

test('byte-drop hex input and hex output', async ({ page }) => {
  await page.goto('/tools/byte-drop/');
  await page.fill('#in-input', '00112233');
  await page.fill('#in-start', '1');
  await page.fill('#in-length', '2');
  await page.selectOption('#in-format', 'hex');
  await page.selectOption('#in-output', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('0033', {
    timeout: 15000,
  });
});

test('byte-drop base64 input rendered as text', async ({ page }) => {
  await page.goto('/tools/byte-drop/');
  await page.fill('#in-input', 'SGVsbG8=');
  await page.fill('#in-start', '0');
  await page.fill('#in-length', '1');
  await page.selectOption('#in-format', 'base64');
  await page.selectOption('#in-output', 'text');
  // "Hello" -> drop 'H' -> "ello"
  await expect(page.locator('#tool-output')).toHaveText('ello', {
    timeout: 15000,
  });
});

test('byte-drop query-param deep-link prefills the controls', async ({ page }) => {
  await page.goto('/tools/byte-drop/?input=abcdef&start=0&length=3');
  await expect(page.locator('#in-input')).toHaveValue('abcdef', {
    timeout: 15000,
  });
  await expect(page.locator('#in-start')).toHaveValue('0');
  await expect(page.locator('#in-length')).toHaveValue('3');
  await expect(page.locator('#tool-output')).toHaveText('def', {
    timeout: 15000,
  });
});

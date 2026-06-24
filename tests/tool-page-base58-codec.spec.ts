import { test, expect } from './fixtures';

test('base58-codec encodes text with default settings', async ({ page }) => {
  await page.goto('/tools/base58-codec/');
  // Defaults: mode=encode, variant=bitcoin, format=text.
  await page.fill('#in-input', 'Hello World!');
  await expect(page.locator('#tool-output')).toHaveText('2NEpo7TZRRrLZSi2U', {
    timeout: 15000,
  });
});

test('base58-codec decodes back to the original text', async ({ page }) => {
  await page.goto('/tools/base58-codec/');
  await page.fill('#in-input', '2NEpo7TZRRrLZSi2U');
  await page.selectOption('#in-mode', 'decode');
  await expect(page.locator('#tool-output')).toHaveText('Hello World!', {
    timeout: 15000,
  });
});

test('base58-codec hex data format encodes raw bytes', async ({ page }) => {
  await page.goto('/tools/base58-codec/');
  await page.fill('#in-input', '516b6fcd0f');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('ABnLTmg', {
    timeout: 15000,
  });
});

test('base58-codec preserves leading zero bytes as 1s', async ({ page }) => {
  await page.goto('/tools/base58-codec/');
  await page.fill('#in-input', '0x0000287fb4cd');
  await page.selectOption('#in-format', 'hex');
  // Two leading 0x00 bytes -> two leading '1's.
  await expect(page.locator('#tool-output')).toHaveText('11233QC4', {
    timeout: 15000,
  });
});

test('base58-codec ripple alphabet differs from bitcoin', async ({ page }) => {
  await page.goto('/tools/base58-codec/');
  await page.fill('#in-input', 'Hello');
  await page.selectOption('#in-variant', 'ripple');
  await expect(page.locator('#tool-output')).toHaveText('9wjdvzi', {
    timeout: 15000,
  });
});

test('base58-codec query-param deep-link prefills the controls', async ({ page }) => {
  await page.goto('/tools/base58-codec/?input=Hello+World%21&variant=bitcoin');
  await expect(page.locator('#in-input')).toHaveValue('Hello World!', {
    timeout: 15000,
  });
  await expect(page.locator('#in-variant')).toHaveValue('bitcoin');
  await expect(page.locator('#tool-output')).toHaveText('2NEpo7TZRRrLZSi2U', {
    timeout: 15000,
  });
});

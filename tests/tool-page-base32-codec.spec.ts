import { test, expect } from './fixtures';

test('base32-codec encodes text with default settings', async ({ page }) => {
  await page.goto('/tools/base32-codec/');
  // Defaults: mode=encode, variant=standard, format=text, padding checked.
  await page.fill('#in-input', 'foobar');
  await expect(page.locator('#tool-output')).toHaveText('MZXW6YTBOI======', {
    timeout: 15000,
  });
});

test('base32-codec decodes back to the original text', async ({ page }) => {
  await page.goto('/tools/base32-codec/');
  await page.fill('#in-input', 'MZXW6YTBOI======');
  await page.selectOption('#in-mode', 'decode');
  await expect(page.locator('#tool-output')).toHaveText('foobar', {
    timeout: 15000,
  });
});

test('base32-codec unpadded encode drops the = padding', async ({ page }) => {
  await page.goto('/tools/base32-codec/');
  await page.fill('#in-input', 'foo');
  // Padding is a default-true checkbox — uncheck it for the compact form.
  await page.uncheck('#in-padding');
  await expect(page.locator('#tool-output')).toHaveText('MZXW6', {
    timeout: 15000,
  });
});

test('base32-codec hex variant (base32hex)', async ({ page }) => {
  await page.goto('/tools/base32-codec/');
  await page.fill('#in-input', 'foobar');
  await page.selectOption('#in-variant', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('CPNMUOJ1E8======', {
    timeout: 15000,
  });
});

test('base32-codec hex data format decodes to raw bytes', async ({ page }) => {
  await page.goto('/tools/base32-codec/');
  await page.fill('#in-input', 'MZXW6===');
  await page.selectOption('#in-mode', 'decode');
  await page.selectOption('#in-format', 'hex');
  // 'foo' -> bytes 0x666f6f.
  await expect(page.locator('#tool-output')).toHaveText('666f6f', {
    timeout: 15000,
  });
});

test('base32-codec lowercase checkbox emits a lowercase alphabet', async ({ page }) => {
  await page.goto('/tools/base32-codec/');
  await page.fill('#in-input', 'foobar');
  await page.check('#in-lowercase');
  await page.uncheck('#in-padding');
  await expect(page.locator('#tool-output')).toHaveText('mzxw6ytboi', {
    timeout: 15000,
  });
});

test('base32-codec query-param deep-link prefills the controls', async ({ page }) => {
  await page.goto(
    '/tools/base32-codec/?input=foobar&variant=crockford',
  );
  await expect(page.locator('#in-input')).toHaveValue('foobar', {
    timeout: 15000,
  });
  await expect(page.locator('#in-variant')).toHaveValue('crockford');
  await expect(page.locator('#tool-output')).toHaveText('CSQPYRK1E8', {
    timeout: 15000,
  });
});

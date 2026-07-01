import { test, expect } from './fixtures';

test('base85-codec page encodes Ascii85 text with defaults', async ({ page }) => {
  await page.goto('/tools/base85-codec/');
  await page.fill('#in-input', 'Hello World!');
  await expect(page.locator('#tool-output')).toHaveText('87cURD]i,"Ebo80', { timeout: 15000 });
});

test('base85-codec page decodes Ascii85 text', async ({ page }) => {
  await page.goto('/tools/base85-codec/');
  await page.fill('#in-input', '87cURD]i,"Ebo80');
  await page.selectOption('#in-mode', 'decode');
  await page.selectOption('#in-format', 'text');
  await expect(page.locator('#tool-output')).toHaveText('Hello World!', { timeout: 15000 });
});

test('base85-codec page supports Z85 hex vectors', async ({ page }) => {
  await page.goto('/tools/base85-codec/');
  await page.fill('#in-input', '864fd26fb559f75b');
  await page.selectOption('#in-variant', 'z85');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('HelloWorld', { timeout: 15000 });

  await page.selectOption('#in-mode', 'decode');
  await page.fill('#in-input', 'HelloWorld');
  await expect(page.locator('#tool-output')).toHaveText('864fd26fb559f75b', { timeout: 15000 });
});

test('base85-codec page supports RFC 1924 and Adobe framing', async ({ page }) => {
  await page.goto('/tools/base85-codec/');
  await page.fill('#in-input', 'Man ');
  await page.selectOption('#in-variant', 'rfc1924');
  await expect(page.locator('#tool-output')).toHaveText('O<`^z', { timeout: 15000 });

  await page.selectOption('#in-variant', 'ascii85');
  await page.check('#in-adobe_frame');
  await expect(page.locator('#tool-output')).toHaveText('<~9jqo^~>', { timeout: 15000 });
});

test('base85-codec page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/base85-codec/?input=864fd26fb559f75b&mode=encode&variant=z85&format=hex');
  await expect(page.locator('#in-input')).toHaveValue('864fd26fb559f75b');
  await expect(page.locator('#in-variant')).toHaveValue('z85');
  await expect(page.locator('#in-format')).toHaveValue('hex');
  await expect(page.locator('#tool-output')).toHaveText('HelloWorld', { timeout: 15000 });
});

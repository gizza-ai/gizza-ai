import { test, expect } from './fixtures';

test('base62-codec page encodes UTF-8 text with defaults', async ({ page }) => {
  await page.goto('/tools/base62-codec/');
  await page.fill('#in-input', 'Hello World!');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('T8dgcjRGkZ3aysdN', { timeout: 15000 });
});

test('base62-codec page decodes standard text', async ({ page }) => {
  await page.goto('/tools/base62-codec/');
  await page.fill('#in-input', 'T8dgcjRGkZ3aysdN');
  await page.selectOption('#in-mode', 'decode');
  await page.selectOption('#in-format', 'text');
  await expect(page.locator('#tool-output')).toHaveText('Hello World!', { timeout: 15000 });
});

test('base62-codec page supports number and inverted alphabets', async ({ page }) => {
  await page.goto('/tools/base62-codec/');
  await page.fill('#in-input', '12345');
  await page.selectOption('#in-format', 'number');
  await expect(page.locator('#tool-output')).toHaveText('3D7', { timeout: 15000 });

  await page.selectOption('#in-variant', 'inverted');
  await expect(page.locator('#tool-output')).toHaveText('3d7', { timeout: 15000 });
});

test('base62-codec page preserves binary leading-zero bytes in hex mode', async ({ page }) => {
  await page.goto('/tools/base62-codec/');
  await page.fill('#in-input', '0000287fb4cd');
  await page.selectOption('#in-format', 'hex');
  await expect(page.locator('#tool-output')).toHaveText('00jyw3x', { timeout: 15000 });

  await page.selectOption('#in-mode', 'decode');
  await page.fill('#in-input', '00jyw3x');
  await expect(page.locator('#tool-output')).toHaveText('0000287fb4cd', { timeout: 15000 });
});

test('base62-codec page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/base62-codec/?input=12345&mode=encode&variant=standard&format=number');
  await expect(page.locator('#in-input')).toHaveValue('12345');
  await expect(page.locator('#in-mode')).toHaveValue('encode');
  await expect(page.locator('#in-format')).toHaveValue('number');
  await expect(page.locator('#tool-output')).toHaveText('3D7', { timeout: 15000 });
});

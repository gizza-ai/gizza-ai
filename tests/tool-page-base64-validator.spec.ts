import { test, expect } from './fixtures';

test('base64-validator reports a valid default Base64 string', async ({ page }) => {
  await page.goto('/tools/base64-validator/');
  await page.fill('#in-input', 'SGVsbG8sIHdvcmxkIQ==');
  await expect(page.locator('#tool-output')).toContainText('VALID', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Decoded size: 13 bytes');
  await expect(page.locator('#tool-output')).toContainText('Hello, world!');
});

test('base64-validator flags invalid characters with position', async ({ page }) => {
  await page.goto('/tools/base64-validator/');
  await page.fill('#in-input', 'SGVsbG8s!Q==');
  await expect(page.locator('#tool-output')).toContainText('INVALID', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText("invalid character '!' (U+0021) at position 9");
});

test('base64-validator honours deep-link strict JSON settings', async ({ page }) => {
  const qs = '?input=QUJD%0ARA%3D%3D&variant=standard&padding=required&ignore_whitespace=false&max_line_length=3&output=json';
  await page.goto('/tools/base64-validator/' + qs);
  await expect(page.locator('#in-variant')).toHaveValue('standard', { timeout: 15000 });
  await expect(page.locator('#in-padding')).toHaveValue('required');
  await expect(page.locator('#in-ignore_whitespace')).not.toBeChecked();
  await expect(page.locator('#in-max_line_length')).toHaveValue('3');
  await expect(page.locator('#in-output')).toHaveValue('json');
  await expect(page.locator('#tool-output')).toContainText('"valid": false', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('whitespace');
  await expect(page.locator('#tool-output')).toContainText('line 1 is 4 characters');
});

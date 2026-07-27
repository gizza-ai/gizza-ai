import { test, expect } from './fixtures';

test('qr-paper-backup renders a printable SVG sheet for text input', async ({ page }) => {
  await page.goto('/tools/qr-paper-backup/');
  await page.fill('#in-input', 'paper backup demo');
  await page.selectOption('#in-input_encoding', 'text');
  await page.fill('#in-chunk_bytes', '300');
  await page.fill('#in-columns', '2');
  await page.selectOption('#in-error_correction', 'M');

  await expect(page.locator('#tool-output')).toContainText('<svg xmlns=', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('QR paper backup');
  await expect(page.locator('#tool-output')).toContainText('Part 1 / 1');
  await expect(page.locator('#tool-output')).toContainText('QRB1|1|1|');
});

test('qr-paper-backup deep-link decodes base64 and hides payload text', async ({ page }) => {
  await page.goto(
    '/tools/qr-paper-backup/?input=aGVsbG8gd29ybGQ%3D&input_encoding=base64&chunk_bytes=50&columns=1&error_correction=Q&show_text=false',
  );

  await expect(page.locator('#in-input')).toHaveValue('aGVsbG8gd29ybGQ=', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('<svg xmlns=', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('11 bytes split into 1 QR codes.');
  await expect(page.locator('#tool-output')).not.toContainText('QRB1|1|1|');
});

test('qr-paper-backup splits at the minimum chunk boundary', async ({ page }) => {
  await page.goto('/tools/qr-paper-backup/');
  await page.fill('#in-input', 'a'.repeat(101));
  await page.fill('#in-chunk_bytes', '50');
  await page.fill('#in-columns', '1');
  await page.selectOption('#in-error_correction', 'H');
  await page.uncheck('#in-show_text');

  await expect(page.locator('#tool-output')).toContainText('101 bytes split into 3 QR codes.', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('Part 3 / 3');
});

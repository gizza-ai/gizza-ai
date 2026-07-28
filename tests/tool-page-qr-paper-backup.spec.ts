import { test, expect } from './fixtures';

test('qr-paper-backup renders a printable SVG sheet for text input', async ({ page }) => {
  await page.goto('/tools/qr-paper-backup/');
  await page.fill('#in-input', 'paper backup demo');
  await page.selectOption('#in-input_encoding', 'text');
  await page.fill('#in-chunk_bytes', '300');
  await page.fill('#in-columns', '2');
  await page.selectOption('#in-error_correction', 'M');

  // format="svg" now renders through <img src="data:image/svg+xml;base64,…">
  // instead of dumping markup into #tool-output.
  const img = page.locator('#tool-output-media');
  await expect(img).toBeVisible({ timeout: 15000 });
  const src = await img.getAttribute('src');
  expect(src?.startsWith('data:image/svg+xml;base64,')).toBe(true);

  const svg = Buffer.from(src!.slice('data:image/svg+xml;base64,'.length), 'base64').toString('utf8');
  expect(svg).toContain('<svg xmlns=');
  expect(svg).toContain('QR paper backup');
  expect(svg).toContain('Part 1 / 1');
  expect(svg).toContain('QRB1|1|1|');

  await expect(page.locator('#tool-output-download')).toBeVisible();
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'qr-paper-backup.svg');
});

test('qr-paper-backup deep-link decodes base64 and hides payload text', async ({ page }) => {
  await page.goto(
    '/tools/qr-paper-backup/?input=aGVsbG8gd29ybGQ%3D&input_encoding=base64&chunk_bytes=50&columns=1&error_correction=Q&show_text=false',
  );

  await expect(page.locator('#in-input')).toHaveValue('aGVsbG8gd29ybGQ=', { timeout: 15000 });

  const img = page.locator('#tool-output-media');
  await expect(img).toBeVisible({ timeout: 15000 });
  const src = await img.getAttribute('src');
  expect(src?.startsWith('data:image/svg+xml;base64,')).toBe(true);

  const svg = Buffer.from(src!.slice('data:image/svg+xml;base64,'.length), 'base64').toString('utf8');
  expect(svg).toContain('<svg xmlns=');
  expect(svg).toContain('11 bytes split into 1 QR codes.');
  expect(svg).not.toContain('QRB1|1|1|');
});

test('qr-paper-backup splits at the minimum chunk boundary', async ({ page }) => {
  await page.goto('/tools/qr-paper-backup/');
  await page.fill('#in-input', 'a'.repeat(101));
  await page.fill('#in-chunk_bytes', '50');
  await page.fill('#in-columns', '1');
  await page.selectOption('#in-error_correction', 'H');
  await page.uncheck('#in-show_text');

  const img = page.locator('#tool-output-media');
  await expect(img).toBeVisible({ timeout: 15000 });
  const src = await img.getAttribute('src');
  expect(src?.startsWith('data:image/svg+xml;base64,')).toBe(true);

  const svg = Buffer.from(src!.slice('data:image/svg+xml;base64,'.length), 'base64').toString('utf8');
  expect(svg).toContain('101 bytes split into 3 QR codes.');
  expect(svg).toContain('Part 3 / 3');
});

test('qr-paper-backup clears the stale image and download link on a later error', async ({ page }) => {
  await page.goto('/tools/qr-paper-backup/');
  await page.fill('#in-input', 'paper backup demo');
  await page.selectOption('#in-input_encoding', 'text');
  await page.fill('#in-chunk_bytes', '300');
  await page.fill('#in-columns', '2');
  await page.selectOption('#in-error_correction', 'M');

  // Produce a real result first, so there is something stale to clear.
  const img = page.locator('#tool-output-media');
  await expect(img).toBeVisible({ timeout: 15000 });
  await expect(page.locator('#tool-output-download')).toBeVisible();

  // Clear ONLY the input field (other fields stay populated) — core/src/lib.rs
  // throws "input is empty", which routes to showError(). The previous
  // image and its now-mismatched download link must not survive that.
  await page.fill('#in-input', '');

  await expect(page.locator('#tool-output')).toContainText('input is empty');
  await expect(img).toBeHidden();
  await expect(page.locator('#tool-output-download')).toBeHidden();
});

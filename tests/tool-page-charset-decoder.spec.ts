import { test, expect } from './fixtures';

// /tools/charset-decoder/ decodes pasted hex/base64 bytes with explicit or detected charsets.
test('charset-decoder decodes a UTF-8 hex dump', async ({ page }) => {
  await page.goto('/tools/charset-decoder/');
  await page.fill('#in-input', '48 65 6c 6c 6f 2c 20 77 6f 72 6c 64 21');
  await page.selectOption('#in-input_format', 'hex');
  await page.fill('#in-charset', 'utf-8');
  await page.selectOption('#in-output', 'text');
  await page.selectOption('#in-errors', 'replace');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Hello, world!', { timeout: 15000 });
});

test('charset-decoder honors deep-linked legacy charset and checkbox state', async ({ page }) => {
  const params = new URLSearchParams({
    input: 'fffe48006900',
    input_format: 'hex',
    charset: 'utf-16le',
    output: 'escaped',
    errors: 'replace',
    strip_bom: 'false',
    per_line: 'false',
  });
  await page.goto(`/tools/charset-decoder/?${params.toString()}`);

  await expect(page.locator('#in-strip_bom')).not.toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('\\u{FEFF}Hi', { timeout: 15000 });
});

test('charset-decoder exercises advertised output and line modes', async ({ page }) => {
  await page.goto('/tools/charset-decoder/');
  await page.fill('#in-input', 'cff0e8e2e5f2');
  await page.selectOption('#in-input_format', 'hex');
  await page.fill('#in-charset', 'windows-1251');
  await page.selectOption('#in-output', 'compare');
  await page.selectOption('#in-errors', 'replace');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('→ windows-1251', { timeout: 15000 });
  await expect(out).toContainText('Привет');

  await page.selectOption('#in-input_format', 'base64');
  await page.fill('#in-input', 'SGVsbG8sIHdvcmxkIQ==');
  await page.fill('#in-charset', 'auto');
  await page.selectOption('#in-output', 'report');
  await expect(out).toContainText('input format   base64', { timeout: 15000 });
  await expect(out).toContainText('charset        UTF-8');

  await page.fill('#in-input', '48656c6c6f\n576f726c64');
  await page.selectOption('#in-input_format', 'hex');
  await page.selectOption('#in-output', 'text');
  await page.check('#in-per_line');
  await expect(out).toContainText('Hello\nWorld', { timeout: 15000 });
});

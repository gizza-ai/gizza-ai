import { test, expect } from './fixtures';

const SAMPLE = 'The quick brown fox jumps over the lazy dog. 1234567890';
// Reference vectors from pieroxy's lz-string JS library (byte-compatible).
const HELLO_BASE64 = 'BIUwNmD2Q===';
const HELLO_URI = 'BIUwNmD2Q';

test('lz-string-compress page compresses to base64 by default', async ({ page }) => {
  await page.goto('/tools/lz-string-compress/');

  await page.fill('#in-text', 'Hello');
  const out = page.locator('#tool-output');
  // base64 default → byte-identical to LZString.compressToBase64('Hello').
  await expect(out).toContainText(HELLO_BASE64, { timeout: 15000 });
});

test('lz-string-compress page uri format is url-safe', async ({ page }) => {
  await page.goto('/tools/lz-string-compress/');

  await page.fill('#in-text', 'Hello');
  await page.selectOption('#in-format', 'uri');
  const out = page.locator('#tool-output');
  await expect(out).toContainText(HELLO_URI, { timeout: 15000 });
  // url-safe alphabet never emits +, / or = padding.
  const text = (await out.textContent()) ?? '';
  expect(text).not.toContain('+');
  expect(text).not.toContain('/');
  expect(text).not.toContain('=');
});

test('lz-string-compress page round-trips compress → decompress', async ({ page }) => {
  await page.goto('/tools/lz-string-compress/');

  await page.fill('#in-text', SAMPLE);
  const out = page.locator('#tool-output');
  await expect(out).not.toBeEmpty({ timeout: 15000 });
  const compressed = ((await out.textContent()) ?? '').trim();
  expect(compressed.length).toBeGreaterThan(0);

  // Feed the payload back in with mode=decompress to recover the original.
  await page.fill('#in-text', compressed);
  await page.selectOption('#in-mode', 'decompress');
  await expect(out).toContainText(SAMPLE, { timeout: 15000 });
});

test('lz-string-compress page reports an honest error on garbage decompress', async ({ page }) => {
  await page.goto('/tools/lz-string-compress/');

  // '@' is outside the base64 alphabet — must surface an error, not "".
  await page.fill('#in-text', '@@@@');
  await page.selectOption('#in-mode', 'decompress');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('decompress failed', { timeout: 15000 });
});

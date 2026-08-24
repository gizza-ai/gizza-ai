import { test, expect } from './fixtures';

// Payloads and expected results come from blocks/zstd-decompress/page/meta.toml
// examples, cross-checked against the core tests and CLI output.
const JSON_B64 = 'KLUv/SQ3nQEAcsMLEaBtDNw9rldW0gajapZ9/r8hcx9z4YXGER2Mozpry+BkUNFFuO1RbLAWPcy2vWUA22iyIw==';
const JSON_TEXT = '{"user":"ada","roles":["admin","deploy"],"active":true}';
const JSON_HEX =
  '7b2275736572223a22616461222c22726f6c6573223a5b2261646d696e222c226465706c6f79225d2c22616374697665223a747275657d';

const HEX_PAYLOAD = '28b52ffd240a51000068656c6c6f207a737464cfdb609c';
const SKIP_B64 = 'UCpNGA0AAABnaXp6YS1tZXRhOnYxKLUv/SQKUQAAaGVsbG8genN0ZM/bYJw=';

test('zstd-decompress page decodes the Base64 JSON example exactly', async ({ page }) => {
  await page.goto('/tools/zstd-decompress/');
  await page.fill('#in-data', JSON_B64);
  await page.selectOption('#in-encoding', 'auto');
  await page.selectOption('#in-output', 'text');
  await page.uncheck('#in-stats');
  await page.uncheck('#in-frame_info');

  await expect(page.locator('#tool-output')).toHaveText(JSON_TEXT, { timeout: 15_000 });
});

test('zstd-decompress page honours hex input and base64 output', async ({ page }) => {
  await page.goto('/tools/zstd-decompress/');
  await page.fill('#in-data', HEX_PAYLOAD);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-output', 'text');
  await page.uncheck('#in-stats');
  await page.uncheck('#in-frame_info');

  await expect(page.locator('#tool-output')).toHaveText('hello zstd', { timeout: 15_000 });

  await page.selectOption('#in-output', 'base64');
  await expect(page.locator('#tool-output')).toHaveText('aGVsbG8genN0ZA==', { timeout: 15_000 });
});

test('zstd-decompress page renders hex output with size stats and frame info', async ({ page }) => {
  await page.goto('/tools/zstd-decompress/');
  await page.fill('#in-data', JSON_B64);
  await page.selectOption('#in-encoding', 'base64');
  await page.selectOption('#in-output', 'hex');
  await page.check('#in-stats');
  await page.check('#in-frame_info');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Compressed:   64 bytes', { timeout: 15_000 });
  await expect(out).toContainText('Decompressed: 55 bytes');
  await expect(out).toContainText('Frames:       1 data frame');
  await expect(out).toContainText('Frame 1 — zstd data frame');
  await expect(out).toContainText('Content checksum: 0x');
  await expect(out).toContainText(JSON_HEX);
});

test('zstd-decompress deep link prefills params and computes', async ({ page }) => {
  const params = new URLSearchParams({
    data: SKIP_B64,
    encoding: 'base64',
    output: 'text',
    stats: 'false',
    frame_info: 'true',
  });
  await page.goto(`/tools/zstd-decompress/?${params.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue(SKIP_B64, { timeout: 15_000 });
  await expect(page.locator('#in-encoding')).toHaveValue('base64');
  await expect(page.locator('#in-output')).toHaveValue('text');
  await expect(page.locator('#in-stats')).not.toBeChecked();
  await expect(page.locator('#in-frame_info')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Frame 1 — skippable frame (magic 0x184d2a50)', { timeout: 15_000 });
  await expect(out).toContainText('Frame 2 — zstd data frame');
  await expect(out).toContainText('hello zstd');
});
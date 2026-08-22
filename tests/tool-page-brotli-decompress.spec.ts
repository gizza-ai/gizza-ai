import { test, expect } from './fixtures';

// Payloads and expected results come from blocks/brotli-decompress/page/meta.toml
// examples, cross-checked against `gizza tool brotli-decompress`.
const JSON_B64 = 'GzYA6IzTFfMjhHUPwW2pr1bT4W8HRZOIyaLT1YpIQtBEajXzAUMiA3t2HQ3lRtbqeY1Rlg8=';
const JSON_TEXT = '{"user":"ada","roles":["admin","deploy"],"active":true}';
const JSON_HEX =
  '7b2275736572223a22616461222c22726f6c6573223a5b2261646d696e222c226465706c6f79225d2c22616374697665223a747275657d';

const HEX_PAYLOAD = '8b058068656c6c6f2062726f746c6903';
const PLAIN_B64 = 'GysA+I2Uq+1oIeQ2tq5jdXdSFGgj2IALmMMAaIVzkVOIetprwilOGRMU84eSAg==';
const PLAIN_TEXT = 'Brotli shrinks repeated text extremely well.';

test('brotli-decompress page decodes the Base64 JSON example exactly', async ({ page }) => {
  await page.goto('/tools/brotli-decompress/');
  await page.fill('#in-data', JSON_B64);
  await page.selectOption('#in-encoding', 'auto');
  await page.selectOption('#in-output', 'text');
  await page.uncheck('#in-stats');

  await expect(page.locator('#tool-output')).toHaveText(JSON_TEXT, { timeout: 15_000 });
});

test('brotli-decompress page honours the non-default hex input encoding', async ({ page }) => {
  await page.goto('/tools/brotli-decompress/');
  await page.fill('#in-data', HEX_PAYLOAD);
  await page.selectOption('#in-encoding', 'hex');
  await page.selectOption('#in-output', 'text');
  await page.uncheck('#in-stats');

  await expect(page.locator('#tool-output')).toHaveText('hello brotli', { timeout: 15_000 });

  // Same hex payload, non-default base64 output.
  await page.selectOption('#in-output', 'base64');
  await expect(page.locator('#tool-output')).toHaveText('aGVsbG8gYnJvdGxp', { timeout: 15_000 });
});

test('brotli-decompress page renders hex output with size and ratio stats', async ({ page }) => {
  await page.goto('/tools/brotli-decompress/');
  await page.fill('#in-data', JSON_B64);
  await page.selectOption('#in-encoding', 'auto');
  await page.selectOption('#in-output', 'hex');
  await page.check('#in-stats');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Compressed:   53 bytes', { timeout: 15_000 });
  await expect(out).toContainText('Decompressed: 55 bytes');
  await expect(out).toContainText('Ratio:        1.04x (decompressed / compressed)');
  await expect(out).toContainText('Space saved:  3.6%');
  await expect(out).toContainText(JSON_HEX);
});

test('brotli-decompress deep link prefills params and computes', async ({ page }) => {
  const params = new URLSearchParams({
    data: PLAIN_B64,
    encoding: 'base64',
    output: 'text',
    stats: 'false',
  });
  await page.goto(`/tools/brotli-decompress/?${params.toString()}`);

  await expect(page.locator('#in-data')).toHaveValue(PLAIN_B64, { timeout: 15_000 });
  await expect(page.locator('#in-encoding')).toHaveValue('base64');
  await expect(page.locator('#in-output')).toHaveValue('text');
  await expect(page.locator('#in-stats')).not.toBeChecked();

  await expect(page.locator('#tool-output')).toHaveText(PLAIN_TEXT, { timeout: 15_000 });
});

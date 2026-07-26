import { test, expect } from './fixtures';

// {"compact": true, "schema": 0} — the canonical MessagePack sample.
const COMPACT_HEX = '82a7636f6d70616374c3a6736368656d6100';
// Base64 of the same bytes.
const COMPACT_B64 = 'gqdjb21wYWN0w6ZzY2hlbWEA';

test('msgpack-to-json decodes hex into pretty JSON', async ({ page }) => {
  await page.goto('/tools/msgpack-to-json/');
  await page.fill('#in-input', COMPACT_HEX);

  await expect(page.locator('#tool-output')).toHaveText(
    '{\n  "compact": true,\n  "schema": 0\n}',
    { timeout: 15000 },
  );
});

test('msgpack-to-json minifies with indent 0', async ({ page }) => {
  await page.goto('/tools/msgpack-to-json/');
  await page.fill('#in-input', COMPACT_HEX);
  await page.selectOption('#in-input_format', 'hex');
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toHaveText(
    '{"compact":true,"schema":0}',
    { timeout: 15000 },
  );
});

test('msgpack-to-json accepts base64 input', async ({ page }) => {
  await page.goto('/tools/msgpack-to-json/');
  await page.fill('#in-input', COMPACT_B64);
  await page.selectOption('#in-input_format', 'base64');
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toHaveText(
    '{"compact":true,"schema":0}',
    { timeout: 15000 },
  );
});

test('msgpack-to-json deep-link decodes a timestamp ext to ISO', async ({ page }) => {
  const params = new URLSearchParams({
    input: 'd6ff5a497a00',
    input_format: 'hex',
    indent: '0',
    binary_format: 'base64',
  });

  await page.goto(`/tools/msgpack-to-json/?${params.toString()}`);
  await expect(page.locator('#in-input_format')).toHaveValue('hex');

  await expect(page.locator('#tool-output')).toHaveText(
    '"2018-01-01T00:00:00Z"',
    { timeout: 15000 },
  );
});

test('msgpack-to-json shows bin payload as hex when selected', async ({ page }) => {
  // bin8 length 3: 0xc4 0x03 0x01 0x02 0x03 → the bytes 01 02 03.
  const params = new URLSearchParams({
    input: 'c403010203',
    input_format: 'hex',
    indent: '0',
    binary_format: 'hex',
  });

  await page.goto(`/tools/msgpack-to-json/?${params.toString()}`);
  await expect(page.locator('#in-binary_format')).toHaveValue('hex');

  await expect(page.locator('#tool-output')).toHaveText('"010203"', {
    timeout: 15000,
  });
});

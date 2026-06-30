import { test, expect } from './fixtures';

test('parse-websocket-frame decodes an unmasked text frame (default json)', async ({ page }) => {
  await page.goto('/tools/parse-websocket-frame/');
  // FIN=1, opcode=1 (text), unmasked, len 5: "Hello".
  await page.fill('#in-input', '81 05 48 65 6c 6c 6f');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"opcode_name": "text"', { timeout: 15000 });
  await expect(out).toContainText('"payload_text": "Hello"');
  await expect(out).toContainText('"masked": false');
});

test('parse-websocket-frame unmasks a masked client frame (RFC 6455 §5.7)', async ({ page }) => {
  await page.goto('/tools/parse-websocket-frame/');
  await page.fill('#in-input', '81 85 37 fa 21 3d 7f 9f 4d 51 58');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"masking_key": "37fa213d"', { timeout: 15000 });
  await expect(out).toContainText('"payload_text": "Hello"');
  await expect(out).toContainText('"masked": true');
});

test('parse-websocket-frame text format decodes a close frame', async ({ page }) => {
  await page.goto('/tools/parse-websocket-frame/?format=text');
  // close frame, status 1000 "normal" + reason "bye".
  await page.fill('#in-input', '88 05 03 e8 62 79 65');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('opcode:  0x8 (close)', { timeout: 15000 });
  await expect(out).toContainText('close code: 1000');
  await expect(out).toContainText('close reason: bye');
});

test('parse-websocket-frame decodes base64 input', async ({ page }) => {
  await page.goto('/tools/parse-websocket-frame/?encoding=base64');
  // base64 of 81 05 48 65 6c 6c 6f.
  await page.fill('#in-input', 'gQVIZWxsbw==');
  await expect(page.locator('#tool-output')).toContainText('"payload_text": "Hello"', {
    timeout: 15000,
  });
});

test('parse-websocket-frame query-param deep-link prefills and parses', async ({ page }) => {
  await page.goto('/tools/parse-websocket-frame/?input=8900&format=text');
  await expect(page.locator('#in-input')).toHaveValue('8900', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('opcode:  0x9 (ping)', {
    timeout: 15000,
  });
});

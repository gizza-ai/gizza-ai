import { test, expect } from './fixtures';

test('parse-grpc-frame splits a single frame and decodes the protobuf (default json)', async ({ page }) => {
  await page.goto('/tools/parse-grpc-frame/');
  // flag 00 | length 00 00 00 03 | payload 08 96 01 = message { 1: 150 }.
  await page.fill('#in-input', '00 00 00 00 03 08 96 01');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"frame_count": 1', { timeout: 15000 });
  await expect(out).toContainText('"length": 3');
  await expect(out).toContainText('"uint": 150');
});

test('parse-grpc-frame text format splits two concatenated frames', async ({ page }) => {
  await page.goto('/tools/parse-grpc-frame/?format=text');
  // {1:150} then {2:42}.
  await page.fill('#in-input', '0000000003089601 0000000002102a');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 gRPC frame(s)', { timeout: 15000 });
  await expect(out).toContainText('frame 0: flag=0 (uncompressed), length=3');
  await expect(out).toContainText('1: varint 150');
  await expect(out).toContainText('frame 1: flag=0 (uncompressed), length=2');
  await expect(out).toContainText('2: varint 42');
});

test('parse-grpc-frame reports a compressed frame without decoding it', async ({ page }) => {
  await page.goto('/tools/parse-grpc-frame/?format=text');
  // flag 01 | length 2 | payload ab cd.
  await page.fill('#in-input', '01 00 00 00 02 ab cd');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('frame 0: flag=1 (compressed), length=2', { timeout: 15000 });
  await expect(out).toContainText('compressed payload');
});

test('parse-grpc-frame decodes base64 input', async ({ page }) => {
  await page.goto('/tools/parse-grpc-frame/?encoding=base64');
  // base64 of 00 00 00 00 03 08 96 01.
  await page.fill('#in-input', 'AAAAAAMIlgE=');
  await expect(page.locator('#tool-output')).toContainText('"uint": 150', {
    timeout: 15000,
  });
});

test('parse-grpc-frame query-param deep-link prefills and parses', async ({ page }) => {
  await page.goto('/tools/parse-grpc-frame/?input=0000000003089601&format=text');
  await expect(page.locator('#in-input')).toHaveValue('0000000003089601', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('frame 0: flag=0 (uncompressed), length=3', {
    timeout: 15000,
  });
});

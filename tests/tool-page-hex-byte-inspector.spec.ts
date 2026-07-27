import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const HI_REPORT = 'Bytes:     2\nBits:      16\nHex chars: 4\n\nHex:    4869\nBase64: SGk=\nText:   "Hi"';

test('hex-byte-inspector page — inspects text bytes and exact output', async ({ page }) => {
  await page.goto('/tools/hex-byte-inspector/');
  await page.fill('#in-input', 'Hi');
  await page.selectOption('#in-input_format', 'text');
  await page.fill('#in-group_size', '0');
  await page.uncheck('#in-interpret');
  await expect(page.locator('#tool-output')).toContainText('Hex:    4869', { timeout: 15000 });
  expect(await outputText(page)).toBe(HI_REPORT);
});

test('hex-byte-inspector page — accepts base64 and uppercase hex', async ({ page }) => {
  await page.goto('/tools/hex-byte-inspector/');
  await page.fill('#in-input', 'SGk=');
  await page.selectOption('#in-input_format', 'base64');
  await page.fill('#in-group_size', '4');
  await page.check('#in-uppercase');
  await page.uncheck('#in-interpret');
  await expect(page.locator('#tool-output')).toContainText('Hex:    4869', { timeout: 15000 });
  expect(await outputText(page)).toBe(HI_REPORT);
});

test('hex-byte-inspector page — crypto-size interpretation and grouping', async ({ page }) => {
  await page.goto('/tools/hex-byte-inspector/');
  await page.fill('#in-input', 'ab'.repeat(32));
  await page.selectOption('#in-input_format', 'hex');
  await page.fill('#in-group_size', '4');
  await page.check('#in-interpret');
  await expect(page.locator('#tool-output')).toContainText('Bytes:     32', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('Hex:    abababab abababab');
  await expect(page.locator('#tool-output')).toContainText('SHA-256 / SHA3-256 / BLAKE2s digest');
});

test('hex-byte-inspector page — query-param deep-link prefills and auto-runs', async ({ page }) => {
  await page.goto(
    '/tools/hex-byte-inspector/?input=' +
      encodeURIComponent('0x48 0x69') +
      '&input_format=hex&group_size=0&uppercase=false&interpret=false',
  );
  await expect(page.locator('#in-input')).toHaveValue('0x48 0x69', { timeout: 15000 });
  await expect(page.locator('#in-input_format')).toHaveValue('hex');
  await expect(page.locator('#tool-output')).toContainText('Hex:    4869', { timeout: 15000 });
  expect(await outputText(page)).toBe(HI_REPORT);
});

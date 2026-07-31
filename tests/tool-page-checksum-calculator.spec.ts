import { test, expect } from './fixtures';

test('checksum-calculator computes CRC-32 and verifies the standard check vector', async ({ page }) => {
  await page.goto('/tools/checksum-calculator/');
  await page.fill('#in-text', '123456789');
  await page.selectOption('#in-algorithm', 'crc32');
  await page.selectOption('#in-input_encoding', 'text');
  await page.selectOption('#in-output_format', 'hex');
  await page.fill('#in-expected', 'cbf43926');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('CRC-32: cbf43926', { timeout: 15000 });
  await expect(out).toContainText('Result: MATCH');
});

test('checksum-calculator covers enum choices, encoded inputs, decimal, and uppercase', async ({ page }) => {
  await page.goto('/tools/checksum-calculator/');
  await page.fill('#in-text', 'MTIzNDU2Nzg5');
  await page.selectOption('#in-algorithm', 'crc8');
  await page.selectOption('#in-input_encoding', 'base64');
  await page.selectOption('#in-output_format', 'decimal');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('CRC-8: 244', { timeout: 15000 });

  await page.fill('#in-text', '313233343536373839');
  await page.selectOption('#in-algorithm', 'crc16');
  await page.selectOption('#in-input_encoding', 'hex');
  await page.selectOption('#in-output_format', 'hex');
  await page.check('#in-uppercase');
  await expect(out).toContainText('CRC-16: BB3D', { timeout: 15000 });

  await page.fill('#in-text', '123456789');
  await page.selectOption('#in-algorithm', 'crc32c');
  await page.selectOption('#in-input_encoding', 'text');
  await page.fill('#in-expected', 'deadbeef');
  await expect(out).toContainText('CRC-32C: E3069283', { timeout: 15000 });
  await expect(out).toContainText('Result: MISMATCH');
});

test('checksum-calculator supports deep-linked CRC-32C uppercase verification', async ({ page }) => {
  const params = new URLSearchParams({
    text: '123456789',
    algorithm: 'crc32c',
    input_encoding: 'text',
    output_format: 'hex',
    uppercase: 'true',
    expected: '0xE3069283',
  });
  await page.goto(`/tools/checksum-calculator/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('CRC-32C: E3069283', { timeout: 15000 });
  await expect(out).toContainText('Result: MATCH');
});

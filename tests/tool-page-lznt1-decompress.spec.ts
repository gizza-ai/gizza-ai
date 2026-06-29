import { test, expect } from './fixtures';

// 05b0 084142430020 is one compressed LZNT1 chunk:
// literals "ABC" plus a back-reference length=3, distance=3 => "ABCABC".
test('lznt1-decompress page decodes hex to text', async ({ page }) => {
  await page.goto('/tools/lznt1-decompress/');
  await page.fill('#in-data', '05b0 084142430020');
  await page.selectOption('#in-output_encoding', 'text');
  await expect(page.locator('#tool-output')).toContainText('ABCABC', { timeout: 15000 });
});

test('lznt1-decompress page decodes stored chunk to hex via deep-link', async ({ page }) => {
  await page.goto('/tools/lznt1-decompress/?data=03b000414243&output_encoding=hex');
  await expect(page.locator('#tool-output')).toContainText('414243', { timeout: 15000 });
});

import { test, expect } from './fixtures';

// Exact multi-line outputs (also asserted verbatim in core unit tests + CLI).
const AND_87_101_8BIT =
  'operation: 87 AND 101 (8-bit)\n' +
  'binary   : 0100 0101\n' +
  'octal    : 0o105\n' +
  'decimal  : 69\n' +
  'hex      : 0x45\n' +
  'signed   : 69';

const NOT_0X0F_8BIT =
  'operation: NOT 0x0F (8-bit)\n' +
  'binary   : 1111 0000\n' +
  'octal    : 0o360\n' +
  'decimal  : 240\n' +
  'hex      : 0xf0\n' +
  'signed   : -16';

test('bitwise-calculator page computes 87 AND 101 at 8 bits exactly', async ({ page }) => {
  await page.goto('/tools/bitwise-calculator/');
  await page.fill('#in-a', '87');
  await page.fill('#in-b', '101');
  await page.selectOption('#in-op', 'and');
  await page.selectOption('#in-bits', '8');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('(8-bit)', { timeout: 15000 });
  expect(await out.textContent()).toBe(AND_87_101_8BIT);
});

test('bitwise-calculator page handles hex input, XOR and the signed reading', async ({ page }) => {
  await page.goto('/tools/bitwise-calculator/');
  await page.fill('#in-a', '0xF0');
  await page.fill('#in-b', '0x0F');
  await page.selectOption('#in-op', 'xor');
  await page.selectOption('#in-bits', '8');
  const out = page.locator('#tool-output');
  // 0xF0 ^ 0x0F = 0xFF = 255 unsigned = -1 signed
  await expect(out).toContainText('decimal  : 255', { timeout: 15000 });
  await expect(out).toContainText('signed   : -1');
});

test('bitwise-calculator page rotates with a wrapping count', async ({ page }) => {
  await page.goto('/tools/bitwise-calculator/');
  await page.fill('#in-a', '0b1000_0001');
  await page.fill('#in-b', '9'); // 9 % 8 == 1
  await page.selectOption('#in-op', 'rotl');
  await page.selectOption('#in-bits', '8');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('binary   : 0000 0011', { timeout: 15000 });
  await expect(out).toContainText('decimal  : 3');
});

test('bitwise-calculator page counts set bits (popcount)', async ({ page }) => {
  await page.goto('/tools/bitwise-calculator/');
  await page.fill('#in-a', '0xDEADBEEF');
  await page.selectOption('#in-op', 'popcount');
  await page.selectOption('#in-bits', '32');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('set bits : 24', { timeout: 15000 });
});

test('bitwise-calculator page honours query-param deep link', async ({ page }) => {
  await page.goto('/tools/bitwise-calculator/?a=0x0F&op=not&bits=8');
  await expect(page.locator('#in-a')).toHaveValue('0x0F');
  await expect(page.locator('#in-op')).toHaveValue('not');
  await expect(page.locator('#in-bits')).toHaveValue('8');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('signed   : -16', { timeout: 15000 });
  expect(await out.textContent()).toBe(NOT_0X0F_8BIT);
});

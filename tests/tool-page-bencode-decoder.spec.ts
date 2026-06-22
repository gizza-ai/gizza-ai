import { test, expect } from './fixtures';

// /tools/bencode-decoder/ converts bencode <-> JSON in-browser (pure wasm).

test('decode bencode to pretty json', async ({ page }) => {
  await page.goto('/tools/bencode-decoder/');
  await page.fill('#in-input', 'd3:cow3:moo4:spam4:eggse');
  await page.selectOption('#in-mode', 'decode');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"cow": "moo"', { timeout: 15000 });
  await expect(out).toContainText('"spam": "eggs"');
});

test('encode json to bencode', async ({ page }) => {
  await page.goto('/tools/bencode-decoder/');
  await page.fill('#in-input', '{"cow":"moo","spam":"eggs"}');
  await page.selectOption('#in-mode', 'encode');
  const out = page.locator('#tool-output');
  // Keys are sorted by byte value: cow before spam.
  await expect(out).toHaveText('d3:cow3:moo4:spam4:eggse', { timeout: 15000 });
});

test('decode without pretty (checkbox off)', async ({ page }) => {
  await page.goto('/tools/bencode-decoder/');
  await page.fill('#in-input', 'l4:spami42ee');
  await page.selectOption('#in-mode', 'decode');
  await page.uncheck('#in-pretty');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('["spam",42]', { timeout: 15000 });
});

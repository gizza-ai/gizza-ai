import { test, expect } from './fixtures';

// /tools/frequency-distribution/ builds a char/byte/n-gram frequency table
// in-browser (pure wasm). input is a multiline <textarea>; input_kind and mode
// are <select>; n is a number field.
test('frequency-distribution ranks characters by frequency with percentages', async ({ page }) => {
  await page.goto('/tools/frequency-distribution/');
  await page.fill('#in-input', 'aabbbc');
  const out = page.locator('#tool-output');
  // b is most frequent (3 = 50%), then a (2 = 33.33%), then c (1).
  await expect(out).toContainText('50.00%  b', { timeout: 15000 });
  await expect(out).toContainText('33.33%  a');
  await expect(out).toContainText('3 distinct symbols, 6 total');
});

test('frequency-distribution counts bigrams in ngram mode', async ({ page }) => {
  await page.goto('/tools/frequency-distribution/');
  await page.fill('#in-input', 'abab');
  await page.selectOption('#in-mode', 'ngram');
  await page.fill('#in-n', '2');
  const out = page.locator('#tool-output');
  // windows ab, ba, ab → ab:2 (66.67%), ba:1.
  await expect(out).toContainText('66.67%  ab', { timeout: 15000 });
  await expect(out).toContainText('33.33%  ba');
});

test('frequency-distribution groups case when case-sensitive is off', async ({ page }) => {
  await page.goto('/tools/frequency-distribution/');
  await page.fill('#in-input', 'AaAa');
  // Default checkbox is checked (case sensitive) → A and a separate.
  const out = page.locator('#tool-output');
  await expect(out).toContainText('2 distinct symbols, 4 total', { timeout: 15000 });
  // Uncheck → fold case → a single symbol.
  await page.uncheck('#in-case_sensitive');
  await expect(out).toContainText('1 distinct symbols, 4 total', { timeout: 15000 });
  await expect(out).toContainText('100.00%  a');
});

test('frequency-distribution tallies bytes from a hex string', async ({ page }) => {
  await page.goto('/tools/frequency-distribution/');
  await page.fill('#in-input', 'ff00ff');
  await page.selectOption('#in-input_kind', 'hex');
  await page.selectOption('#in-mode', 'byte');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('0xff', { timeout: 15000 });
  await expect(out).toContainText('0x00');
  await expect(out).toContainText('2 distinct symbols, 3 total');
});

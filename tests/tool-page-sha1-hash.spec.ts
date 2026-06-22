import { test, expect } from './fixtures';

const ABC_HEX = 'a9993e364706816aba3e25717850c26c9cd0d89d';

test('sha1-hash page hashes text (default hex)', async ({ page }) => {
  await page.goto('/tools/sha1-hash/');
  await page.fill('#in-text', 'abc');
  await expect(page.locator('#tool-output')).toHaveText(ABC_HEX, {
    timeout: 15000,
  });
});

test('sha1-hash base64 output format', async ({ page }) => {
  await page.goto('/tools/sha1-hash/');
  await page.fill('#in-text', 'abc');
  await page.selectOption('#in-output_format', 'base64');
  await expect(page.locator('#tool-output')).toHaveText(
    'qZk+NkcGgWq6PiVxeFDCbJzQ2J0=',
    { timeout: 15000 },
  );
});

test('sha1-hash uppercase checkbox', async ({ page }) => {
  await page.goto('/tools/sha1-hash/');
  await page.fill('#in-text', 'abc');
  await page.check('#in-uppercase');
  await expect(page.locator('#tool-output')).toHaveText(ABC_HEX.toUpperCase(), {
    timeout: 15000,
  });
});

test('sha1-hash hex input encoding matches text', async ({ page }) => {
  await page.goto('/tools/sha1-hash/');
  // "abc" as hex is 616263 — decoding then hashing equals hashing "abc".
  await page.fill('#in-text', '616263');
  await page.selectOption('#in-input_encoding', 'hex');
  await expect(page.locator('#tool-output')).toHaveText(ABC_HEX, {
    timeout: 15000,
  });
});

test('sha1-hash query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/sha1-hash/?text=abc&output_format=base64');
  await expect(page.locator('#in-text')).toHaveValue('abc', { timeout: 15000 });
  await expect(page.locator('#in-output_format')).toHaveValue('base64');
  await expect(page.locator('#tool-output')).toHaveText(
    'qZk+NkcGgWq6PiVxeFDCbJzQ2J0=',
    { timeout: 15000 },
  );
});

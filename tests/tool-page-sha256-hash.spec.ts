import { test, expect } from './fixtures';

const ABC_HEX =
  'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';

test('sha256-hash page hashes text (default hex)', async ({ page }) => {
  await page.goto('/tools/sha256-hash/');
  await page.fill('#in-text', 'abc');
  await expect(page.locator('#tool-output')).toHaveText(ABC_HEX, {
    timeout: 15000,
  });
});

test('sha256-hash base64 output format', async ({ page }) => {
  await page.goto('/tools/sha256-hash/');
  await page.fill('#in-text', 'abc');
  await page.selectOption('#in-output_format', 'base64');
  await expect(page.locator('#tool-output')).toHaveText(
    'ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=',
    { timeout: 15000 },
  );
});

test('sha256-hash uppercase checkbox', async ({ page }) => {
  await page.goto('/tools/sha256-hash/');
  await page.fill('#in-text', 'abc');
  await page.check('#in-uppercase');
  await expect(page.locator('#tool-output')).toHaveText(ABC_HEX.toUpperCase(), {
    timeout: 15000,
  });
});

test('sha256-hash hex input encoding matches text', async ({ page }) => {
  await page.goto('/tools/sha256-hash/');
  // "abc" as hex is 616263 — decoding then hashing equals hashing "abc".
  await page.fill('#in-text', '616263');
  await page.selectOption('#in-input_encoding', 'hex');
  await expect(page.locator('#tool-output')).toHaveText(ABC_HEX, {
    timeout: 15000,
  });
});

test('sha256-hash query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/sha256-hash/?text=abc&output_format=base64',
  );
  await expect(page.locator('#in-text')).toHaveValue('abc', { timeout: 15000 });
  await expect(page.locator('#in-output_format')).toHaveValue('base64');
  await expect(page.locator('#tool-output')).toHaveText(
    'ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=',
    { timeout: 15000 },
  );
});

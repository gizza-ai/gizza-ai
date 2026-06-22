import { test, expect } from './fixtures';

const ABC_SHA256 =
  'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';

test('hash-text page hashes text (default sha256 hex)', async ({ page }) => {
  await page.goto('/tools/hash-text/');
  await page.fill('#in-text', 'abc');
  await expect(page.locator('#tool-output')).toHaveText(ABC_SHA256, {
    timeout: 15000,
  });
});

test('hash-text md5 algorithm', async ({ page }) => {
  await page.goto('/tools/hash-text/');
  await page.fill('#in-text', 'abc');
  await page.selectOption('#in-algorithm', 'md5');
  await expect(page.locator('#tool-output')).toHaveText(
    '900150983cd24fb0d6963f7d28e17f72',
    { timeout: 15000 },
  );
});

test('hash-text blake3 algorithm', async ({ page }) => {
  await page.goto('/tools/hash-text/');
  await page.fill('#in-text', 'abc');
  await page.selectOption('#in-algorithm', 'blake3');
  await expect(page.locator('#tool-output')).toHaveText(
    '6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85',
    { timeout: 15000 },
  );
});

test('hash-text base64 output format', async ({ page }) => {
  await page.goto('/tools/hash-text/');
  await page.fill('#in-text', 'abc');
  await page.selectOption('#in-output_format', 'base64');
  await expect(page.locator('#tool-output')).toHaveText(
    'ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=',
    { timeout: 15000 },
  );
});

test('hash-text uppercase checkbox', async ({ page }) => {
  await page.goto('/tools/hash-text/');
  await page.fill('#in-text', 'abc');
  await page.check('#in-uppercase');
  await expect(page.locator('#tool-output')).toHaveText(
    ABC_SHA256.toUpperCase(),
    { timeout: 15000 },
  );
});

test('hash-text hex input encoding matches text', async ({ page }) => {
  await page.goto('/tools/hash-text/');
  // "abc" as hex is 616263 — decoding then hashing equals hashing "abc".
  await page.fill('#in-text', '616263');
  await page.selectOption('#in-input_encoding', 'hex');
  await expect(page.locator('#tool-output')).toHaveText(ABC_SHA256, {
    timeout: 15000,
  });
});

test('hash-text query-param deep-link prefills and computes', async ({
  page,
}) => {
  await page.goto('/tools/hash-text/?text=abc&algorithm=sha3-256');
  await expect(page.locator('#in-text')).toHaveValue('abc', { timeout: 15000 });
  await expect(page.locator('#in-algorithm')).toHaveValue('sha3-256');
  await expect(page.locator('#tool-output')).toHaveText(
    '3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532',
    { timeout: 15000 },
  );
});

import { test, expect } from './fixtures';

// SHA3-256("abc") — the FIPS-202 standard vector, distinct from Keccak-256.
const ABC_SHA3_256 =
  '3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532';

test('sha3-hash page hashes text (default sha3-256 hex)', async ({ page }) => {
  await page.goto('/tools/sha3-hash/');
  await page.fill('#in-text', 'abc');
  await expect(page.locator('#tool-output')).toHaveText(ABC_SHA3_256, {
    timeout: 15000,
  });
});

test('sha3-hash sha3-512 variant', async ({ page }) => {
  await page.goto('/tools/sha3-hash/');
  await page.fill('#in-text', 'abc');
  await page.selectOption('#in-algorithm', 'sha3-512');
  await expect(page.locator('#tool-output')).toHaveText(
    'b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0',
    { timeout: 15000 },
  );
});

test('sha3-hash uppercase checkbox', async ({ page }) => {
  await page.goto('/tools/sha3-hash/');
  await page.fill('#in-text', 'abc');
  await page.check('#in-uppercase');
  await expect(page.locator('#tool-output')).toHaveText(
    ABC_SHA3_256.toUpperCase(),
    { timeout: 15000 },
  );
});

test('sha3-hash hex input (0x prefix) matches text', async ({ page }) => {
  await page.goto('/tools/sha3-hash/');
  // "abc" as hex is 616263 — decoding then hashing equals hashing "abc".
  await page.fill('#in-text', '0x616263');
  await page.selectOption('#in-input_encoding', 'hex');
  await expect(page.locator('#tool-output')).toHaveText(ABC_SHA3_256, {
    timeout: 15000,
  });
});

test('sha3-hash query-param deep-link prefills and computes', async ({
  page,
}) => {
  await page.goto('/tools/sha3-hash/?text=abc&algorithm=sha3-256');
  await expect(page.locator('#in-text')).toHaveValue('abc', { timeout: 15000 });
  await expect(page.locator('#in-algorithm')).toHaveValue('sha3-256');
  await expect(page.locator('#tool-output')).toHaveText(ABC_SHA3_256, {
    timeout: 15000,
  });
});

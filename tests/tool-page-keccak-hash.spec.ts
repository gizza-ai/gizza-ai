import { test, expect } from './fixtures';

// Keccak-256("abc") — the original-Keccak (Ethereum) padding, distinct from SHA3-256.
const ABC_KECCAK256 =
  '4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45';

test('keccak-hash page hashes text (default keccak-256 hex)', async ({
  page,
}) => {
  await page.goto('/tools/keccak-hash/');
  await page.fill('#in-text', 'abc');
  await expect(page.locator('#tool-output')).toHaveText(ABC_KECCAK256, {
    timeout: 15000,
  });
});

test('keccak-hash keccak-512 variant', async ({ page }) => {
  await page.goto('/tools/keccak-hash/');
  await page.fill('#in-text', 'abc');
  await page.selectOption('#in-algorithm', 'keccak-512');
  await expect(page.locator('#tool-output')).toHaveText(
    '18587dc2ea106b9a1563e32b3312421ca164c7f1f07bc922a9c83d77cea3a1e5d0c69910739025372dc14ac9642629379540c17e2a65b19d77aa511a9d00bb96',
    { timeout: 15000 },
  );
});

test('keccak-hash uppercase checkbox', async ({ page }) => {
  await page.goto('/tools/keccak-hash/');
  await page.fill('#in-text', 'abc');
  await page.check('#in-uppercase');
  await expect(page.locator('#tool-output')).toHaveText(
    ABC_KECCAK256.toUpperCase(),
    { timeout: 15000 },
  );
});

test('keccak-hash hex input (0x prefix) matches text', async ({ page }) => {
  await page.goto('/tools/keccak-hash/');
  // "abc" as hex is 616263 — decoding then hashing equals hashing "abc".
  await page.fill('#in-text', '0x616263');
  await page.selectOption('#in-input_encoding', 'hex');
  await expect(page.locator('#tool-output')).toHaveText(ABC_KECCAK256, {
    timeout: 15000,
  });
});

test('keccak-hash query-param deep-link prefills and computes', async ({
  page,
}) => {
  await page.goto('/tools/keccak-hash/?text=abc&algorithm=keccak-256');
  await expect(page.locator('#in-text')).toHaveValue('abc', { timeout: 15000 });
  await expect(page.locator('#in-algorithm')).toHaveValue('keccak-256');
  await expect(page.locator('#tool-output')).toHaveText(ABC_KECCAK256, {
    timeout: 15000,
  });
});

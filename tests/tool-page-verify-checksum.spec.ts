import { test, expect } from './fixtures';

const ABC_SHA256 =
  'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';
const ABC_MD5 = '900150983cd24fb0d6963f7d28e17f72';

test('verify-checksum page reports MATCH (auto-detected sha256)', async ({
  page,
}) => {
  await page.goto('/tools/verify-checksum/');
  await page.fill('#in-text', 'abc');
  await page.fill('#in-expected', ABC_SHA256);
  await expect(page.locator('#tool-output')).toContainText('status: MATCH', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('algorithm: sha256');
});

test('verify-checksum page reports MISMATCH', async ({ page }) => {
  await page.goto('/tools/verify-checksum/');
  await page.fill('#in-text', 'abcd');
  await page.fill('#in-expected', ABC_SHA256);
  await page.selectOption('#in-algorithm', 'sha256');
  await expect(page.locator('#tool-output')).toContainText('status: MISMATCH', {
    timeout: 15000,
  });
});

test('verify-checksum page auto-detects md5 by length', async ({ page }) => {
  await page.goto('/tools/verify-checksum/');
  await page.fill('#in-text', 'abc');
  await page.fill('#in-expected', ABC_MD5);
  await expect(page.locator('#tool-output')).toContainText('algorithm: md5', {
    timeout: 15000,
  });
  await expect(page.locator('#tool-output')).toContainText('status: MATCH');
});

test('verify-checksum page hex input encoding matches text', async ({
  page,
}) => {
  await page.goto('/tools/verify-checksum/');
  // "abc" as hex is 616263 — decoding then hashing equals hashing "abc".
  await page.fill('#in-text', '616263');
  await page.fill('#in-expected', ABC_SHA256);
  await page.selectOption('#in-input_encoding', 'hex');
  await expect(page.locator('#tool-output')).toContainText('status: MATCH', {
    timeout: 15000,
  });
});

test('verify-checksum query-param deep-link prefills and verifies', async ({
  page,
}) => {
  await page.goto(
    `/tools/verify-checksum/?text=abc&expected=${ABC_SHA256}&algorithm=sha256`,
  );
  await expect(page.locator('#in-text')).toHaveValue('abc', { timeout: 15000 });
  await expect(page.locator('#in-expected')).toHaveValue(ABC_SHA256);
  await expect(page.locator('#tool-output')).toContainText('status: MATCH');
});

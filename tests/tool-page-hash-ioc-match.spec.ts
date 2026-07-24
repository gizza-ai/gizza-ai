import { test, expect } from './fixtures';

const ABC_MD5 = '900150983cd24fb0d6963f7d28e17f72';
const ABC_SHA256 =
  'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';

test('hash-ioc-match page reports FLAGGED on an MD5 blocklist hit', async ({
  page,
}) => {
  await page.goto('/tools/hash-ioc-match/');
  await page.fill('#in-input', 'abc');
  // Labelled + CSV blocklist line — parsed leniently to the bare MD5.
  await page.fill('#in-blocklist', `MD5: ${ABC_MD5} , dropper.exe`);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('status: FLAGGED', { timeout: 15000 });
  await expect(out).toContainText('<-- MATCH');
  await expect(out).toContainText(ABC_MD5);
});

test('hash-ioc-match page reports CLEAN when nothing matches', async ({
  page,
}) => {
  await page.goto('/tools/hash-ioc-match/');
  await page.fill('#in-input', 'hello');
  // A real hash, but of a different file — no digest of "hello" matches it.
  await page.fill('#in-blocklist', ABC_MD5);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('status: CLEAN', { timeout: 15000 });
  await expect(out).not.toContainText('<-- MATCH');
});

test('hash-ioc-match query-param deep-link prefills blocklist + input_encoding', async ({
  page,
}) => {
  // "abc" as base64 is "YWJj"; decoding then hashing equals hashing "abc",
  // so its SHA-256 matches the blocklist and the file is FLAGGED.
  await page.goto(
    `/tools/hash-ioc-match/?input=YWJj&blocklist=${ABC_SHA256}&input_encoding=base64`,
  );
  await expect(page.locator('#in-input')).toHaveValue('YWJj', {
    timeout: 15000,
  });
  await expect(page.locator('#in-blocklist')).toHaveValue(ABC_SHA256);
  await expect(page.locator('#in-input_encoding')).toHaveValue('base64');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('status: FLAGGED');
  await expect(out).toContainText('sha256');
});

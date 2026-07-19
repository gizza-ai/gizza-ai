import { test, expect } from './fixtures';

// /tools/iban-extractor-validator/ scans text for IBANs and validates each with
// the ISO 13616 mod-97 checksum, in-browser (pure wasm).

const INVOICE =
  'Please remit payment to GB82 WEST 1234 5698 7654 32. Our old account DE89 3704 0044 0532 0130 00 is no longer in use.';

const INVOICE_EXPECTED = `Found 2 IBAN(s): 2 valid, 0 invalid.

Valid (2):
  GB82 WEST 1234 5698 7654 32  -  United Kingdom
  DE89 3704 0044 0532 0130 00  -  Germany`;

test('iban-extractor-validator extracts + validates two IBANs (exact output)', async ({ page }) => {
  await page.goto('/tools/iban-extractor-validator/');
  await page.fill('#in-text', INVOICE);
  // Wait for the run, then compare the raw multi-line text exactly.
  await expect(page.locator('#tool-output')).toContainText('2 valid, 0 invalid', {
    timeout: 15000,
  });
  const text = (await page.locator('#tool-output').textContent())?.trim();
  expect(text).toBe(INVOICE_EXPECTED);
});

test('iban-extractor-validator flags a checksum typo as invalid', async ({ page }) => {
  await page.goto('/tools/iban-extractor-validator/');
  // Valid French IBAN + a UK IBAN with the final digit corrupted (31 not 32).
  await page.fill(
    '#in-text',
    'Beneficiary IBAN: FR14 2004 1010 0505 0001 3M02 606 — backup GB82 WEST 1234 5698 7654 31',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1 valid, 1 invalid', { timeout: 15000 });
  await expect(out).toContainText('FR14 2004 1010 0505 0001 3M02 606  -  France');
  await expect(out).toContainText('GB82 WEST 1234 5698 7654 31  -  failed the mod-97 checksum');
});

test('iban-extractor-validator reports when none present', async ({ page }) => {
  await page.goto('/tools/iban-extractor-validator/');
  await page.fill('#in-text', 'just an ordinary sentence with no bank details in it');
  await expect(page.locator('#tool-output')).toContainText('No IBANs found', { timeout: 15000 });
});

test('iban-extractor-validator deep-link pre-fills and runs', async ({ page }) => {
  const q = encodeURIComponent('Contiguous DE89370400440532013000 works too.');
  await page.goto(`/tools/iban-extractor-validator/?text=${q}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('1 valid, 0 invalid', { timeout: 15000 });
  await expect(out).toContainText('DE89 3704 0044 0532 0130 00  -  Germany');
});

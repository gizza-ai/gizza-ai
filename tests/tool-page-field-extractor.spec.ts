import { test, expect } from './fixtures';

const TABLE = 'alice 30 engineer\nbob 25 designer\ncarol 41 writer';
const CSV = 'id,name,email\n1,Ada,ada@x.io\n2,Alan,alan@x.io';

test('field-extractor pulls whitespace columns with defaults', async ({ page }) => {
  await page.goto('/tools/field-extractor/');
  await page.fill('#in-text', TABLE);
  await page.fill('#in-selectors', '1,3');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('alice engineer\nbob designer\ncarol writer', { timeout: 15000 });
});

test('field-extractor deep-link takes the last CSV column and skips the header', async ({ page }) => {
  await page.goto('/tools/field-extractor/?mode=fields&selectors=-1&delimiter=,&skip_header=true');
  await page.fill('#in-text', CSV);

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('ada@x.io\nalan@x.io', { timeout: 15000 });
});

test('field-extractor reorders columns with a tab output delimiter', async ({ page }) => {
  await page.goto('/tools/field-extractor/');
  await page.fill('#in-text', 'Ada,Lovelace,36\nAlan,Turing,41');
  await page.fill('#in-selectors', '2,1');
  await page.fill('#in-delimiter', ',');
  await page.fill('#in-output_delimiter', '\\t');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('Lovelace\tAda\nTuring\tAlan', { timeout: 15000 });
});

test('field-extractor extracts a character range (Unicode-safe)', async ({ page }) => {
  await page.goto('/tools/field-extractor/?mode=chars&selectors=1-4');
  await page.fill('#in-text', 'PROD-001\nTEST-999\nÜBER-042');

  const out = page.locator('#tool-output');
  await expect(out).toHaveText('PROD\nTEST\nÜBER', { timeout: 15000 });
});

test('field-extractor reports a clear error for a zero selector', async ({ page }) => {
  await page.goto('/tools/field-extractor/');
  await page.fill('#in-text', TABLE);
  await page.fill('#in-selectors', '0');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('cannot be 0', { timeout: 15000 });
});

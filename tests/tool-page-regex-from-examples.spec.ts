import { test, expect } from './fixtures';

// /tools/regex-from-examples/ infers a verified regex from sample strings (pure wasm).
test('regex-from-examples infers an anchored date pattern from positive examples', async ({ page }) => {
  await page.goto('/tools/regex-from-examples/');
  await page.fill('#in-examples', '2024-01-15\n2023-11-02\n1999-12-31');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('^\\d{4}-\\d{2}-\\d{2}$', { timeout: 15000 });
});

test('regex-from-examples deep link renders the verification report with negatives', async ({ page }) => {
  await page.goto('/tools/regex-from-examples/?examples=2024-01-15%0A2023-11-02%0A1999-12-31&negatives=2024%2F01%2F15%0Anot-a-date&output=report');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('^\\d{4}-\\d{2}-\\d{2}$', { timeout: 15000 });
  await expect(out).toContainText('Strategy: generalize (3 example(s), 1 shape(s))');
  await expect(out).toContainText('exactly 4 digits');
  await expect(out).toContainText('examples:  3/3 match');
  await expect(out).toContainText('negatives: 2/2 excluded');
});

// Non-default checkbox (case_insensitive) + non-default enum (flavor) driven through
// the real form controls — a marshaling bug would send every checkbox as "on".
test('regex-from-examples emits a JavaScript literal with the case-insensitive flag', async ({ page }) => {
  await page.goto('/tools/regex-from-examples/');
  await page.fill('#in-examples', 'AB-12\nCD-345');
  await page.selectOption('#in-flavor', 'javascript');
  await page.check('#in-case_insensitive');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('/^[A-Z]{2}-\\d{2,3}$/i', { timeout: 15000 });

  // Unchecking must drop the flag again (checkbox state really round-trips).
  await page.uncheck('#in-case_insensitive');
  await expect(out).toContainText('/^[A-Z]{2}-\\d{2,3}$/', { timeout: 15000 });
  await expect(out).not.toContainText('$/i');
});

// Non-default enum path: alternation strategy factors shared prefixes into a trie.
test('regex-from-examples builds a literal alternation when the strategy is forced', async ({ page }) => {
  await page.goto('/tools/regex-from-examples/');
  await page.fill('#in-examples', 'foobar\nfoobaz\nfooza\nfoozap');
  await page.selectOption('#in-strategy', 'alternation');
  await page.selectOption('#in-output', 'report');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('^foo(?:ba[rz]|zap?)$', { timeout: 15000 });
  await expect(out).toContainText('Strategy: alternation (4 example(s), 1 shape(s))');
  await expect(out).toContainText('examples:  4/4 match');
});

// Non-default separator enum + json output, with anchors turned off.
test('regex-from-examples deep link supports comma separator and JSON output', async ({ page }) => {
  await page.goto('/tools/regex-from-examples/?examples=AB-12%2CCD-345&separator=comma&output=json&anchors=false');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"pattern": "[A-Z]{2}-\\\\d{2,3}"', { timeout: 15000 });
  await expect(out).toContainText('"anchored": false');
  await expect(out).toContainText('"example_count": 2');
});

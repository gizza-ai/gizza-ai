import { test, expect } from './fixtures';

const DOC = [
  'Shipping policy: orders are dispatched within two days.',
  'Send returns to our adress on file and include the receipt.',
  'Refunds are processed within five working days.',
].join('\n');

// Exact output: a fuzzy (one-typo) match. "address" is edit-distance 1 from the
// document's misspelled "adress", so it matches at the default fuzziness of 1
// with a below-100 score, and the matched word is wrapped in «…».
test('fuzzy-doc-search page finds a typo and ranks it', async ({ page }) => {
  await page.goto('/tools/fuzzy-doc-search/');
  await page.fill('#in-query', 'address');
  await page.fill('#in-text', DOC);
  await expect(page.locator('#tool-output')).toContainText('1 match for', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe(
    '1 match for "address"\n\n#1  line 2  score 70\nSend returns to our «adress» on file and include the receipt.'
  );
});

// Deep-link: params prefill the fields and the tool auto-computes. Exact-word
// match scores 100.
test('fuzzy-doc-search deep-link prefills and computes', async ({ page }) => {
  await page.goto('/tools/fuzzy-doc-search/?query=beta&text=alpha+beta+gamma');
  await expect(page.locator('#in-query')).toHaveValue('beta', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('1 match for', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toBe('1 match for "beta"\n\n#1  line 1  score 100\nalpha «beta» gamma');
});

// Enum "match": ALL (AND) keeps only snippets that contain every query word.
test('fuzzy-doc-search match=all requires every word', async ({ page }) => {
  await page.goto('/tools/fuzzy-doc-search/');
  await page.fill('#in-query', 'alpha beta');
  await page.fill('#in-text', 'alpha beta here\nonly alpha here');
  await page.selectOption('#in-match', 'all');
  await expect(page.locator('#tool-output')).toContainText('1 match for', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('#1  line 1  score 100');
  expect(out).toContain('«alpha» «beta»');
  expect(out).not.toContain('line 2');
});

// Enum "unit": sentence granularity (split on . ! ?).
test('fuzzy-doc-search unit=sentence segments on sentences', async ({ page }) => {
  await page.goto('/tools/fuzzy-doc-search/');
  await page.fill('#in-query', 'dogs');
  await page.fill('#in-text', 'Cats sleep all day. Dogs bark at night.');
  await page.selectOption('#in-unit', 'sentence');
  await expect(page.locator('#tool-output')).toContainText('sentence 2', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('«Dogs»');
});

// Enum "unit": paragraph granularity (split on blank lines).
test('fuzzy-doc-search unit=paragraph segments on blank lines', async ({ page }) => {
  await page.goto('/tools/fuzzy-doc-search/');
  await page.fill('#in-query', 'dogs');
  await page.fill('#in-text', 'A paragraph about cats.\n\nA paragraph about dogs.');
  await page.selectOption('#in-unit', 'paragraph');
  await expect(page.locator('#tool-output')).toContainText('paragraph 2', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('«dogs»');
});

// Non-default checkbox: case_sensitive on, fuzziness off — only "Rust" matches,
// "rust" is highlighted-excluded within the same line.
test('fuzzy-doc-search case_sensitive checkbox distinguishes case', async ({ page }) => {
  await page.goto('/tools/fuzzy-doc-search/');
  await page.fill('#in-query', 'Rust');
  await page.fill('#in-text', 'Rust and rust');
  await page.fill('#in-fuzziness', '0');
  await page.check('#in-case_sensitive');
  await expect(page.locator('#tool-output')).toContainText('1 match for', {
    timeout: 15000,
  });
  const out = await page.locator('#tool-output').textContent();
  expect(out).toContain('«Rust» and rust');
  expect(out).not.toContain('«rust»');
});

// Non-default checkbox: whole_word on — "doc" no longer matches inside
// "Documentation".
test('fuzzy-doc-search whole_word checkbox disables substring match', async ({ page }) => {
  await page.goto('/tools/fuzzy-doc-search/');
  await page.fill('#in-query', 'doc');
  await page.fill('#in-text', 'Documentation is here');
  await page.fill('#in-fuzziness', '0');
  await page.check('#in-whole_word');
  const out = page.locator('#tool-output');
  await expect(out).toHaveText('No matches for "doc".', { timeout: 15000 });
});

// Cap boundary: 60 matching lines, max_results at the cap (50) and one over (51,
// clamped to 50) both report "showing top 50".
test('fuzzy-doc-search max_results caps at 50', async ({ page }) => {
  const many = Array(60).fill('cat').join('\n');
  await page.goto('/tools/fuzzy-doc-search/');
  await page.fill('#in-query', 'cat');
  await page.fill('#in-text', many);
  await page.fill('#in-max_results', '50');
  await expect(page.locator('#tool-output')).toContainText(
    '60 matches for "cat" (showing top 50)',
    { timeout: 15000 }
  );
  // One over the cap is clamped, not an error.
  await page.fill('#in-max_results', '51');
  await expect(page.locator('#tool-output')).toContainText(
    '60 matches for "cat" (showing top 50)',
    { timeout: 15000 }
  );
});

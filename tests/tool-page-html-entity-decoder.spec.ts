import { test, expect } from './fixtures';

// /tools/html-entity-decoder/ decodes HTML character references back into the
// characters they stand for, entirely in-browser (pure wasm). Field ids are
// in-<name>: #in-text is the multiline input, #in-unknown is a <select> whose
// options (keep/error) come from the descriptor schema. Output lands in
// #tool-output. Single-line inputs are used for exact-match assertions because
// Playwright's toHaveText normalizes internal newlines/whitespace.

test('html-entity-decoder page: named entities decode to characters', async ({ page }) => {
  await page.goto('/tools/html-entity-decoder/');
  await page.fill('#in-text', 'Fish &amp; Chips &mdash; caf&eacute;');
  await expect(page.locator('#tool-output')).toHaveText(
    'Fish & Chips — café',
    { timeout: 15000 },
  );
});

test('html-entity-decoder page: decimal numeric references decode', async ({ page }) => {
  await page.goto('/tools/html-entity-decoder/');
  await page.fill('#in-text', '&#169; 2026 &#8364;5');
  await expect(page.locator('#tool-output')).toHaveText('© 2026 €5', {
    timeout: 15000,
  });
});

test('html-entity-decoder page: hex numeric references decode (case-insensitive)', async ({ page }) => {
  await page.goto('/tools/html-entity-decoder/');
  await page.fill('#in-text', '&#xA9; &#X2122;');
  await expect(page.locator('#tool-output')).toHaveText('© ™', {
    timeout: 15000,
  });
});

test('html-entity-decoder page: Windows-1252 remap for C1 numeric references', async ({ page }) => {
  await page.goto('/tools/html-entity-decoder/');
  // &#151; is an em dash and &#128; the euro sign under the WHATWG remap.
  await page.fill('#in-text', '&#147;quote&#148; &#151; &#128;5');
  await expect(page.locator('#tool-output')).toHaveText('“quote” — €5', {
    timeout: 15000,
  });
});

test('html-entity-decoder page: legacy entity decodes without a semicolon', async ({ page }) => {
  await page.goto('/tools/html-entity-decoder/');
  // The legacy set decodes even without ';', preserving the trailing text.
  await page.fill('#in-text', '&copyright 2026');
  await expect(page.locator('#tool-output')).toHaveText('©right 2026', {
    timeout: 15000,
  });
});

test('html-entity-decoder page: unknown=keep leaves bad references untouched', async ({ page }) => {
  await page.goto('/tools/html-entity-decoder/');
  // Default select value is keep: the unknown name and bare '&' survive verbatim.
  await page.fill('#in-text', 'a &notareal; b &amp; c');
  await expect(page.locator('#tool-output')).toHaveText('a &notareal; b & c', {
    timeout: 15000,
  });
});

test('html-entity-decoder page: unknown=error names the offending reference', async ({ page }) => {
  await page.goto('/tools/html-entity-decoder/');
  await page.fill('#in-text', 'good &amp; &notreal; bad');
  await page.selectOption('#in-unknown', 'error');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('&notreal;', { timeout: 15000 });
  await expect(out).toHaveClass(/error/);
});

test('html-entity-decoder page: query-param deep-link prefills and decodes', async ({ page }) => {
  await page.goto(
    '/tools/html-entity-decoder/?text=' +
      encodeURIComponent('Fish &amp; Chips &#169; &#x2122;') +
      '&unknown=keep',
  );
  await expect(page.locator('#in-text')).toHaveValue(
    'Fish &amp; Chips &#169; &#x2122;',
    { timeout: 15000 },
  );
  await expect(page.locator('#tool-output')).toHaveText('Fish & Chips © ™', {
    timeout: 15000,
  });
});

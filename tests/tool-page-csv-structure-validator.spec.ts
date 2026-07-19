import { test, expect } from './fixtures';

// /tools/csv-structure-validator/ lints raw CSV text in-browser (pure wasm):
// ragged rows, unclosed/stray quotes, blank rows, header checks, whitespace.

const BROKEN = 'name,email,city\nAda,ada@example.com,Paris,extra\nBo,bo"x,Rome\nCy,"cy@example.com';

const BROKEN_REPORT = [
  'INVALID CSV — 3 error(s), 0 warning(s).',
  'Delimiter: comma (auto-detected) · Quote: double · Expected 3 field(s) per row · 3 data row(s)',
  'Line 2 [error] ragged_row — expected 3 field(s), found 4',
  'Line 3 [error] stray_quote — unquoted field 2 contains a bare quote character — wrap the field in quotes and double any embedded quote',
  'Line 4 [error] unclosed_quote — quoted field 2 is opened here but never closed',
].join('\n');

test('csv-structure-validator reports the exact broken-CSV report', async ({ page }) => {
  await page.goto('/tools/csv-structure-validator/');
  await page.fill('#in-data', BROKEN);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID CSV', { timeout: 15000 });
  // Multi-line output: compare textContent exactly (toHaveText normalizes whitespace).
  expect(await out.textContent()).toBe(BROKEN_REPORT);
});

test('csv-structure-validator clean CSV is valid (exact report)', async ({ page }) => {
  await page.goto('/tools/csv-structure-validator/');
  await page.fill('#in-data', 'a,b\n1,2');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid CSV', { timeout: 15000 });
  expect(await out.textContent()).toBe(
    'Valid CSV — no structural problems found.\nDelimiter: comma (auto-detected) · Quote: double · Expected 2 field(s) per row · 1 data row(s)'
  );
});

test('csv-structure-validator deep-link prefills and runs (semicolon enum)', async ({ page }) => {
  await page.goto(
    '/tools/csv-structure-validator/?data=a%3Bb%3Bc%0A1%3B2&delimiter=semicolon'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID CSV — 1 error(s), 0 warning(s).', { timeout: 15000 });
  await expect(out).toContainText('Delimiter: semicolon · Quote: double');
  await expect(out).toContainText('Line 2 [error] ragged_row — expected 3 field(s), found 2');
  await expect(page.locator('#in-delimiter')).toHaveValue('semicolon');
});

test('csv-structure-validator delimiter select: tab, comma, pipe', async ({ page }) => {
  await page.goto('/tools/csv-structure-validator/');
  await page.fill('#in-data', 'a\tb\n1\t2\t3');
  await page.selectOption('#in-delimiter', 'tab');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Delimiter: tab · Quote: double', { timeout: 15000 });
  await expect(out).toContainText('ragged_row — expected 2 field(s), found 3');

  // Same data under an explicit comma delimiter: one column, no delimiters → ragged single-field rows.
  await page.selectOption('#in-delimiter', 'comma');
  await expect(out).toContainText('Delimiter: comma · Quote: double', { timeout: 15000 });

  await page.fill('#in-data', 'a|b\n1|2');
  await page.selectOption('#in-delimiter', 'pipe');
  await expect(out).toContainText('Delimiter: pipe · Quote: double', { timeout: 15000 });
  await expect(out).toContainText('Valid CSV — no structural problems found.');
});

test('csv-structure-validator quote select: single and none', async ({ page }) => {
  await page.goto('/tools/csv-structure-validator/');
  await page.fill('#in-data', "a,b\n'x,y',2");
  await page.selectOption('#in-quote', 'single');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid CSV — no structural problems found.', { timeout: 15000 });
  await expect(out).toContainText('Quote: single');

  await page.selectOption('#in-quote', 'none');
  await expect(out).toContainText('INVALID CSV — 1 error(s)', { timeout: 15000 });
  await expect(out).toContainText('Quote: none');
  await expect(out).toContainText('ragged_row — expected 2 field(s), found 3');
});

test('csv-structure-validator header checkbox off disables header checks', async ({ page }) => {
  await page.goto('/tools/csv-structure-validator/');
  await page.fill('#in-data', 'id,,id\n1,2,3');
  const out = page.locator('#tool-output');
  // Default (checked): empty + duplicate header names are warnings.
  await expect(out).toContainText('Valid CSV — no errors, 2 warning(s).', { timeout: 15000 });
  await expect(out).toContainText('empty_header — header column(s) 2 have an empty name');
  await expect(out).toContainText("duplicate_header — duplicate header name 'id' (columns 1 and 3)");
  // NON-default checkbox state: header off → no header checks, 2 data rows.
  await page.uncheck('#in-header');
  await expect(out).toContainText('Valid CSV — no structural problems found.', { timeout: 15000 });
  await expect(out).toContainText('2 data row(s)');
});

test('csv-structure-validator comment char and max_issues cap boundary', async ({ page }) => {
  await page.goto('/tools/csv-structure-validator/');
  await page.fill('#in-data', '# note\na,b\n1,2');
  await page.fill('#in-comment', '#');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid CSV — no structural problems found.', { timeout: 15000 });

  // Cap boundary: max_issues=1 lists one issue but counts all three errors.
  await page.fill('#in-comment', '');
  await page.fill('#in-data', 'a,b\n1\n2\n3');
  await page.fill('#in-max_issues', '1');
  await expect(out).toContainText('INVALID CSV — 3 error(s), 0 warning(s).', { timeout: 15000 });
  await expect(out).toContainText('Line 2 [error] ragged_row');
  await expect(out).toContainText('(+ 2 more issue(s) not shown — raise max_issues to list them)');
});

test('csv-structure-validator warnings: blank/empty rows and whitespace', async ({ page }) => {
  // NOTE: mixed_line_endings can't be exercised through the page — browser
  // textareas normalize CRLF to LF. It is covered by core tests and the CLI.
  await page.goto('/tools/csv-structure-validator/');
  await page.fill('#in-data', 'a,b\n1,2\n\n,\n 3,4');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid CSV — no errors, 3 warning(s).', { timeout: 15000 });
  await expect(out).toContainText('[warning] blank_row — blank line');
  await expect(out).toContainText('empty_row — all 2 field(s) are empty');
  await expect(out).toContainText('whitespace — field(s) 1 have leading or trailing space(s)');
});

test('csv-structure-validator example chip prefills and runs the worked example', async ({
  page,
}) => {
  await page.goto('/tools/csv-structure-validator/');
  await page.click('button.tool-example-chip[data-example="0"]');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('INVALID CSV — 3 error(s), 0 warning(s).', { timeout: 15000 });
  await expect(out).toContainText('unclosed_quote — quoted field 2 is opened here but never closed');
  await expect(page.locator('#in-delimiter')).toHaveValue('auto');
});

test('csv-structure-validator empty input shows a graceful error', async ({ page }) => {
  await page.goto('/tools/csv-structure-validator/');
  await page.fill('#in-data', '   ');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('input is empty', { timeout: 15000 });
  await expect(out).toHaveClass(/error/);
});

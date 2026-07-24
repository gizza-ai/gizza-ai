import { test, expect } from './fixtures';

// /tools/csv-to-ndjson/ converts CSV text to NDJSON (one JSON value per line)
// entirely in-browser (pure wasm). Field ids are in-<name>; the headers
// checkbox is pre-checked (schema default true), the inference/trim checkboxes
// default off. Single-row inputs are used for exact-match assertions because
// Playwright's toHaveText normalizes internal newlines.

test('csv-to-ndjson page: default emits string-valued objects', async ({ page }) => {
  await page.goto('/tools/csv-to-ndjson/');
  // headers pre-checked, no inference → every value stays a JSON string.
  await page.fill('#in-data', 'name,age\nAda,36');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"name":"Ada","age":"36"}',
    { timeout: 15000 },
  );
});

test('csv-to-ndjson page: multi-row NDJSON is one object per line', async ({ page }) => {
  await page.goto('/tools/csv-to-ndjson/');
  await page.fill('#in-data', 'name,age\nAda,36\nGrace,');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('{"name":"Ada","age":"36"}', { timeout: 15000 });
  await expect(out).toContainText('{"name":"Grace","age":""}');
});

test('csv-to-ndjson page: query-param deep-link prefills + types numbers', async ({ page }) => {
  await page.goto(
    '/tools/csv-to-ndjson/?data=' +
      encodeURIComponent('name,age\nAda,36') +
      '&parse_numbers=true',
  );
  await expect(page.locator('#in-data')).toHaveValue('name,age\nAda,36', {
    timeout: 15000,
  });
  await expect(page.locator('#in-parse_numbers')).toBeChecked();
  // headers untouched → still an object; age coerced to a JSON number.
  await expect(page.locator('#tool-output')).toHaveText(
    '{"name":"Ada","age":36}',
    { timeout: 15000 },
  );
});

test('csv-to-ndjson page: tab delimiter parses TSV columns', async ({ page }) => {
  await page.goto('/tools/csv-to-ndjson/');
  await page.fill('#in-data', 'a\tb\n1\t2');
  await page.fill('#in-delimiter', 'tab');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"a":"1","b":"2"}',
    { timeout: 15000 },
  );
});

test('csv-to-ndjson page: semicolon delimiter parses columns', async ({ page }) => {
  await page.goto('/tools/csv-to-ndjson/');
  await page.fill('#in-data', 'a;b\nx;y');
  await page.fill('#in-delimiter', ';');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"a":"x","b":"y"}',
    { timeout: 15000 },
  );
});

test('csv-to-ndjson page: headers off emits JSON arrays', async ({ page }) => {
  await page.goto('/tools/csv-to-ndjson/');
  await page.fill('#in-data', 'a,b');
  await page.uncheck('#in-headers');
  await expect(page.locator('#tool-output')).toHaveText('["a","b"]', {
    timeout: 15000,
  });
});

test('csv-to-ndjson page: parse true/false/null coerces literals and empties', async ({ page }) => {
  await page.goto('/tools/csv-to-ndjson/');
  await page.fill('#in-data', 'active,note\ntrue,');
  await page.check('#in-parse_bools');
  // "true" → boolean, empty cell → null.
  await expect(page.locator('#tool-output')).toHaveText(
    '{"active":true,"note":null}',
    { timeout: 15000 },
  );
});

test('csv-to-ndjson page: trim + parse numbers strips padding then types', async ({ page }) => {
  await page.goto('/tools/csv-to-ndjson/');
  await page.fill('#in-data', 'name, age\n Ada , 36 ');
  await page.check('#in-trim');
  await page.check('#in-parse_numbers');
  await expect(page.locator('#tool-output')).toHaveText(
    '{"name":"Ada","age":36}',
    { timeout: 15000 },
  );
});

test('csv-to-ndjson page: leading zeros survive number parsing as strings', async ({ page }) => {
  await page.goto('/tools/csv-to-ndjson/');
  await page.fill('#in-data', 'zip\n007');
  await page.check('#in-parse_numbers');
  await expect(page.locator('#tool-output')).toHaveText('{"zip":"007"}', {
    timeout: 15000,
  });
});

test('csv-to-ndjson page: invalid multi-char delimiter surfaces an error', async ({ page }) => {
  await page.goto('/tools/csv-to-ndjson/');
  await page.fill('#in-data', 'a,b\n1,2');
  await page.fill('#in-delimiter', 'xx');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('single character', { timeout: 15000 });
  await expect(out).toHaveClass(/error/);
});

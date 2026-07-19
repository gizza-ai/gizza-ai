import { test, expect } from './fixtures';

// /tools/data-anonymizer/ redacts identifier columns, generalizes
// quasi-identifiers (numeric bins / date->year / text prefix masking) and
// reports the achieved k-anonymity — pure wasm, in-browser. output is a
// <select>; suppress / dates_to_year / header are checkboxes; k / text_keep
// are sliders (canonical #in-<name> number box).

// Multi-line output: compare textContent exactly (toHaveText normalizes whitespace).
async function outText(page): Promise<string> {
  return (await page.locator('#tool-output').textContent()) ?? '';
}

const HOSPITAL =
  'name,age,zipcode,gender,diagnosis\n' +
  'Ada,34,13053,F,Flu\n' +
  'Bea,38,13068,F,Cold\n' +
  'Cal,52,14850,M,Flu\n' +
  'Dan,57,14853,M,Fever';

test('hospital demo: redact + per-column override + full report (output=both)', async ({ page }) => {
  await page.goto('/tools/data-anonymizer/');
  await page.fill('#in-data', HOSPITAL);
  await page.fill('#in-quasi', 'age,zipcode:100,gender');
  await page.fill('#in-identifiers', 'name');
  await page.fill('#in-sensitive', 'diagnosis');
  await expect(page.locator('#tool-output')).toContainText('K-anonymity report', { timeout: 15000 });
  expect(await outText(page)).toBe(
    'name,age,zipcode,gender,diagnosis\n' +
      '[REDACTED],30-39,13000-13099,F,Flu\n' +
      '[REDACTED],30-39,13000-13099,F,Cold\n' +
      '[REDACTED],50-59,14800-14899,M,Flu\n' +
      '[REDACTED],50-59,14800-14899,M,Fever\n' +
      '\n' +
      'K-anonymity report\n' +
      'Data rows: 4 (0 suppressed)\n' +
      'Quasi-identifiers: age, zipcode, gender\n' +
      'Redacted identifiers: name\n' +
      'Equivalence classes: 2 (smallest 2, largest 2)\n' +
      'Achieved k = 2 — every row is indistinguishable from at least 1 other row(s)\n' +
      'Target k = 2: MET — no rows fall below the target\n' +
      "Distinct l-diversity on 'diagnosis': l = 2\n"
  );
});

test('output=csv with a non-default bin width returns only the CSV', async ({ page }) => {
  await page.goto('/tools/data-anonymizer/');
  await page.fill('#in-data', 'age\n34\n36');
  await page.fill('#in-quasi', 'age');
  await page.fill('#in-numeric_bin', '5');
  await page.selectOption('#in-output', 'csv');
  await expect(page.locator('#tool-output')).toContainText('30-34', { timeout: 15000 });
  expect(await outText(page)).toBe('age\n30-34\n35-39\n');
});

test('suppress checkbox (non-default) drops under-k rows, output=report', async ({ page }) => {
  await page.goto('/tools/data-anonymizer/');
  await page.fill('#in-data', 'age\n34\n36\n71');
  await page.fill('#in-quasi', 'age');
  await page.check('#in-suppress'); // default off; test the on-path
  await page.selectOption('#in-output', 'report');
  await expect(page.locator('#tool-output')).toContainText('1 suppressed', { timeout: 15000 });
  expect(await outText(page)).toBe(
    'K-anonymity report\n' +
      'Data rows: 2 (1 suppressed)\n' +
      'Quasi-identifiers: age\n' +
      'Equivalence classes: 1 (smallest 2, largest 2)\n' +
      'Achieved k = 2 — every row is indistinguishable from at least 1 other row(s)\n' +
      'Target k = 2: MET — no rows fall below the target\n'
  );
});

test('dates_to_year off-path (non-default checkbox) + text_keep slider', async ({ page }) => {
  await page.goto('/tools/data-anonymizer/');
  await page.fill('#in-data', 'dob\n1987-04-12\n1987-11-30');
  await page.fill('#in-quasi', 'dob');
  await page.selectOption('#in-output', 'csv');
  // default: ISO dates generalize to the year
  await expect(page.locator('#tool-output')).toContainText('1987', { timeout: 15000 });
  expect(await outText(page)).toBe('dob\n1987\n1987\n');
  // off-path: the column falls back to text prefix masking (keep 2 chars)
  await page.uncheck('#in-dates_to_year');
  await page.fill('#in-text_keep', '2');
  await expect(page.locator('#tool-output')).toContainText('19********', { timeout: 15000 });
  expect(await outText(page)).toBe('dob\n19********\n19********\n');
});

test('deep-link pre-fills and auto-runs (?data&quasi&output=report)', async ({ page }) => {
  const data = encodeURIComponent('age\n34\n36\n71');
  await page.goto(`/tools/data-anonymizer/?data=${data}&quasi=age&output=report`);
  await expect(page.locator('#tool-output')).toContainText('NOT MET', { timeout: 15000 });
  expect(await outText(page)).toBe(
    'K-anonymity report\n' +
      'Data rows: 3 (0 suppressed)\n' +
      'Quasi-identifiers: age\n' +
      'Equivalence classes: 2 (smallest 1, largest 2)\n' +
      'Achieved k = 1 — at least one row is unique on its quasi-identifiers\n' +
      'Target k = 2: NOT MET — 1 of 3 rows (33.3%) are in classes smaller than 2\n'
  );
});

// Set a huge textarea value like a paste: page.fill routes 20k newlines through
// Chromium insertText (minutes); value + input event is what fill dispatches
// anyway and is what the driver's field listener reacts to.
async function setBigField(page, selector: string, value: string) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('row cap: exactly 10000 rows runs, 10001 errors', async ({ page }) => {
  await page.goto('/tools/data-anonymizer/');
  await page.fill('#in-quasi', 'v');
  await page.selectOption('#in-output', 'report');
  await setBigField(page, '#in-data', 'v\n' + '7\n'.repeat(10000));
  await expect(page.locator('#tool-output')).toContainText('Achieved k = 10000', { timeout: 30000 });
  await setBigField(page, '#in-data', 'v\n' + '7\n'.repeat(10001));
  await expect(page.locator('#tool-output')).toContainText(
    'too many rows: 10001 (max 10000 data rows per run)',
    { timeout: 30000 }
  );
});

test('unknown quasi column shows a graceful error', async ({ page }) => {
  await page.goto('/tools/data-anonymizer/');
  await page.fill('#in-data', 'age\n34\n36');
  await page.fill('#in-quasi', 'height');
  await expect(page.locator('#tool-output')).toContainText(
    "no column named 'height' and it is not a valid index",
    { timeout: 15000 }
  );
});

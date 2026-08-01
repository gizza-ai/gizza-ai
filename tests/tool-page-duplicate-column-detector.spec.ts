import { test, expect } from './fixtures';

// id and id_copy hold identical values; email and contact are identical too;
// name is unique. Two duplicate groups, two redundant columns.
const SAMPLE =
  'id,name,email,id_copy,contact\n1,Alice,a@x.com,1,a@x.com\n2,Bob,b@y.com,2,b@y.com';

test('duplicate-column-detector reports duplicate column groups (keep first)', async ({ page }) => {
  await page.goto('/tools/duplicate-column-detector/');
  await page.fill('#in-data', SAMPLE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Scanned 5 columns across 2 data rows.', { timeout: 15000 });
  await expect(out).toContainText(
    'Found 2 duplicate column groups; 2 redundant columns can be removed (3 columns remain unique).',
  );
  // Whitespace-normalized substrings: the leftmost column of each group is kept.
  await expect(out).toContainText('keep "id" (col 1) == drop "id_copy" (col 4)');
  await expect(out).toContainText('keep "email" (col 3) == drop "contact" (col 5)');
});

test('duplicate-column-detector output=csv removes redundant columns (exact)', async ({ page }) => {
  await page.goto('/tools/duplicate-column-detector/');
  await page.fill('#in-data', SAMPLE);
  await page.selectOption('#in-output', 'csv');
  // Exact multi-line output: redundant id_copy + contact dropped, first kept.
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), { timeout: 15000 })
    .toBe('id,name,email\n1,Alice,a@x.com\n2,Bob,b@y.com');
});

test('duplicate-column-detector output=json deep-link pre-fills and computes', async ({ page }) => {
  await page.goto(
    '/tools/duplicate-column-detector/?data=' + encodeURIComponent(SAMPLE) + '&output=json',
  );
  await expect(page.locator('#in-data')).toHaveValue(SAMPLE, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"columns": 5', { timeout: 15000 });
  await expect(out).toContainText('"duplicate_groups": 2');
  await expect(out).toContainText('"redundant_columns": 2');
  await expect(out).toContainText('"unique_columns": 3');
  await expect(out).toContainText('"column": "id_copy"');
});

test('duplicate-column-detector ignore_header_name off requires matching names', async ({ page }) => {
  // Identical values, different names: dup by default, distinct once names must match.
  const data = 'email,contact\na@x.com,a@x.com\nb@y.com,b@y.com';
  await page.goto('/tools/duplicate-column-detector/');
  await page.fill('#in-data', data);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Found 1 duplicate column group', { timeout: 15000 });
  // Non-default checkbox state: turning OFF "ignore header names" splits them.
  await page.uncheck('#in-ignore_header_name');
  await expect(out).toContainText('No duplicate columns found', { timeout: 15000 });
});

test('duplicate-column-detector tab delimiter enum value', async ({ page }) => {
  // Tab-delimited; first and third columns share values.
  const data = 'a\tb\tc\n1\tx\t1\n2\ty\t2';
  await page.goto('/tools/duplicate-column-detector/');
  await page.fill('#in-data', data);
  await page.selectOption('#in-delimiter', 'tab');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Found 1 duplicate column group', { timeout: 15000 });
  await expect(out).toContainText('keep "a" (col 1) == drop "c" (col 3)');
});

test('duplicate-column-detector normalization checkboxes gate near-identical columns', async ({ page }) => {
  // "Alice"/"alice" and "NY"/"ny " differ only by case/whitespace.
  const data = 'a,b\nAlice,alice\nNY,ny ';
  await page.goto('/tools/duplicate-column-detector/');
  await page.fill('#in-data', data);
  const out = page.locator('#tool-output');
  // Defaults on → the two columns collapse into one duplicate group.
  await expect(out).toContainText('Found 1 duplicate column group', { timeout: 15000 });
  // Turn OFF both normalizers (non-default state) → they no longer match.
  await page.uncheck('#in-ignore_case');
  await page.uncheck('#in-ignore_whitespace');
  await expect(out).toContainText('No duplicate columns found', { timeout: 15000 });
});

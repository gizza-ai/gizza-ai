import { test, expect } from './fixtures';

const SAMPLE = 'name,email\nAlice,a@x.com\nBob,b@y.com\nAlice,a@x.com\nCarol,c@z.com\nBob,b@y.com';

test('duplicate-row-finder page reports whole-row duplicate groups', async ({ page }) => {
  await page.goto('/tools/duplicate-row-finder/');
  await page.fill('#in-data', SAMPLE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Found 2 duplicate groups covering 4 rows', { timeout: 15000 });
  await expect(out).toContainText('x2  Alice | a@x.com   (lines 2, 4)');
  await expect(out).toContainText('x2  Bob | b@y.com   (lines 3, 6)');
});

test('duplicate-row-finder query-param deep-link keyed on a column', async ({ page }) => {
  const data = 'name,email\nAlice,a@x.com\nAli,a@x.com\nBob,b@y.com';
  await page.goto('/tools/duplicate-row-finder/?data=' + encodeURIComponent(data) + '&columns=email&output=report');
  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('keyed on email', { timeout: 15000 });
  await expect(out).toContainText('Found 1 duplicate group');
});

test('duplicate-row-finder output=csv emits only the duplicate rows', async ({ page }) => {
  await page.goto('/tools/duplicate-row-finder/');
  await page.fill('#in-data', SAMPLE);
  await page.selectOption('#in-output', 'csv');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('name,email', { timeout: 15000 });
  await expect(out).toContainText('Alice,a@x.com');
  await expect(out).toContainText('Bob,b@y.com');
  // Carol is unique — must NOT appear in the duplicate-rows export.
  await expect(out).not.toContainText('Carol');
});

test('duplicate-row-finder output=json reports structured groups + column analysis', async ({ page }) => {
  await page.goto('/tools/duplicate-row-finder/');
  await page.fill('#in-data', SAMPLE);
  await page.selectOption('#in-output', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"duplicate_groups": 2', { timeout: 15000 });
  await expect(out).toContainText('"duplicate_rows": 4');
  await expect(out).toContainText('"column_analysis"');
});

test('duplicate-row-finder normalization checkboxes gate near-duplicates', async ({ page }) => {
  // "Alice"/"ALICE" and "a@x.com"/" a@x.com " match only after normalization.
  const near = 'name,email\nAlice,a@x.com\nALICE, a@x.com ';
  await page.goto('/tools/duplicate-row-finder/');
  await page.fill('#in-data', near);
  const out = page.locator('#tool-output');
  // Defaults on → the two rows collapse into one duplicate group.
  await expect(out).toContainText('Found 1 duplicate group', { timeout: 15000 });
  // Turn OFF both normalizers (non-default state) → they no longer match.
  await page.uncheck('#in-ignore_case');
  await page.uncheck('#in-ignore_whitespace');
  await expect(out).toContainText('No duplicate rows found', { timeout: 15000 });
});

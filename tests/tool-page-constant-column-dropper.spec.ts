import { test, expect } from './fixtures';

// `country` is constant ("US" in every row), `notes` is entirely empty,
// `id` and `score` vary. Two zero-variance columns, two survivors.
const SAMPLE = 'id,country,score,notes\n1,US,10,\n2,US,20,\n3,US,30,\n4,US,40,';

test('constant-column-dropper reports constant and all-empty columns', async ({ page }) => {
  await page.goto('/tools/constant-column-dropper/');
  await page.fill('#in-data', SAMPLE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Scanned 4 columns across 4 data rows (dominance 100%).', {
    timeout: 15000,
  });
  await expect(out).toContainText('Found 2 constant columns; 2 columns remain.');
  await expect(out).toContainText('"country" (col 2) = "US" in 4/4 rows (100%)');
  await expect(out).toContainText('"notes" (col 4) = all cells are empty');
});

test('constant-column-dropper output=csv removes the constant columns (exact)', async ({ page }) => {
  await page.goto('/tools/constant-column-dropper/');
  await page.fill('#in-data', SAMPLE);
  await page.selectOption('#in-output', 'csv');
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), { timeout: 15000 })
    .toBe('id,score\n1,10\n2,20\n3,30\n4,40');
});

test('constant-column-dropper json deep-link pre-fills and computes', async ({ page }) => {
  await page.goto(
    '/tools/constant-column-dropper/?data=' + encodeURIComponent(SAMPLE) + '&output=json',
  );
  await expect(page.locator('#in-data')).toHaveValue(SAMPLE, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"dropped_columns": 2', { timeout: 15000 });
  await expect(out).toContainText('"kept_columns": 2');
  await expect(out).toContainText('"top_share_percent": 100.0');
  await expect(out).toContainText('"reason": "all cells are empty"');
});

test('constant-column-dropper dominance slider catches near-constant columns', async ({ page }) => {
  // `flag` is "Y" in 3 of 4 rows = 75% — constant only below a 75% threshold.
  const data = 'id,flag\n1,Y\n2,Y\n3,Y\n4,N';
  await page.goto('/tools/constant-column-dropper/');
  await page.fill('#in-data', data);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('No constant columns found', { timeout: 15000 });
  // The number box is canonical for the slider control kind.
  await page.fill('#in-dominance', '75');
  await expect(out).toContainText('"flag" (col 2) = "Y" in 3/4 rows (75%)', { timeout: 15000 });
});

test('constant-column-dropper empty_cells=ignore makes a sparse column constant', async ({ page }) => {
  const data = 'id,tier\n1,gold\n2,\n3,gold';
  await page.goto('/tools/constant-column-dropper/');
  await page.fill('#in-data', data);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('No constant columns found', { timeout: 15000 });
  await page.selectOption('#in-empty_cells', 'ignore');
  await expect(out).toContainText('"tier" (col 2) = "gold" in 2/2 rows (100%)', { timeout: 15000 });
});

test('constant-column-dropper normalization checkboxes gate a case/space-only column', async ({ page }) => {
  // "NY" / "ny " / " Ny" differ only by case and whitespace.
  const data = 'id,state\n1,NY\n2,ny \n3, Ny';
  await page.goto('/tools/constant-column-dropper/');
  await page.fill('#in-data', data);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"state" (col 2)', { timeout: 15000 });
  // Non-default checkbox states: turn both normalizers OFF.
  await page.uncheck('#in-ignore_case');
  await page.uncheck('#in-ignore_whitespace');
  await expect(out).toContainText('No constant columns found', { timeout: 15000 });
});

test('constant-column-dropper keep protects a constant column', async ({ page }) => {
  await page.goto('/tools/constant-column-dropper/');
  await page.fill('#in-data', SAMPLE);
  await page.fill('#in-keep', 'country');
  await page.selectOption('#in-output', 'csv');
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), { timeout: 15000 })
    .toBe('id,country,score\n1,US,10\n2,US,20\n3,US,30\n4,US,40');
});

test('constant-column-dropper tab delimiter and header off', async ({ page }) => {
  // Headerless, tab-delimited; the second column never changes.
  const data = '1\tUS\n2\tUS\n3\tUS';
  await page.goto('/tools/constant-column-dropper/');
  await page.fill('#in-data', data);
  await page.selectOption('#in-delimiter', 'tab');
  await page.uncheck('#in-header');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Scanned 2 columns across 3 data rows', { timeout: 15000 });
  await expect(out).toContainText('col 2 = "US" in 3/3 rows (100%)');
});

test('constant-column-dropper errors when every column is constant in csv mode', async ({ page }) => {
  await page.goto('/tools/constant-column-dropper/');
  await page.fill('#in-data', 'a,b\nx,1\nx,1');
  await page.selectOption('#in-output', 'csv');
  await expect(page.locator('#tool-output')).toContainText(
    'every column is constant (2 of 2 columns would be dropped)',
    { timeout: 15000 },
  );
});

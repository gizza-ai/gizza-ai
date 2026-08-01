import { test, expect } from './fixtures';

test('numeric-range-check flags out-of-range CSV cells with exact text', async ({ page }) => {
  await page.goto('/tools/numeric-range-check/');
  await page.fill('#in-data', 'name,age\nAda,34\nBo,150\nCy,-3');
  await page.fill('#in-columns', 'age');
  await page.fill('#in-min', '0');
  await page.fill('#in-max', '120');
  await page.selectOption('#in-format', 'text');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('OUT OF RANGE — 2 flagged cell(s).', { timeout: 15000 });
  await expect(out).toContainText('Row 2 (line 3), column "age" — 150 is above max 120');
  await expect(out).toContainText('Row 3 (line 4), column "age" — -3 is below min 0');
});

test('numeric-range-check supports JSON report and all columns', async ({ page }) => {
  await page.goto('/tools/numeric-range-check/');
  await page.fill('#in-data', 'low,high\n5,200\n1,2');
  await page.fill('#in-columns', 'all');
  await page.fill('#in-min', '0');
  await page.fill('#in-max', '10');
  await page.selectOption('#in-format', 'json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": false', { timeout: 15000 });
  await expect(out).toContainText('"offending_count": 1');
  await expect(out).toContainText('"column": "high"');
});

test('numeric-range-check supports deep-linked headerless semicolon data', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'Ada;150\nBo;40',
    columns: '2',
    min: '0',
    max: '120',
    inclusive: 'true',
    header: 'false',
    delimiter: 'semicolon',
    non_numeric: 'flag',
    empty_ok: 'true',
    max_issues: '50',
    format: 'text',
  });
  await page.goto(`/tools/numeric-range-check/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('OUT OF RANGE — 1 flagged cell(s).', { timeout: 15000 });
  await expect(out).toContainText('Row 1 (line 1), column "col 2" — 150 is above max 120');
});

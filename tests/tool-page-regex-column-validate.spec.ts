import { test, expect } from './fixtures';

const ZIP_CSV = 'name,zip\nAda,12345\nBo,1234\nCy,90210';

const TEXT_OUTPUT = `Column "zip" (index 1) checked against /\\d{5}/
Match mode: full match, case-sensitive
Flagging: values that do not match the pattern
Rows checked: 3
Valid: 2
Invalid: 1

Invalid values:
  row 2 (line 3): "1234" — does not match the pattern`;

const JSON_OUTPUT = `{
  "column": "status",
  "column_index": 0,
  "pattern": "[A-Z]+",
  "full_match": true,
  "ignore_case": false,
  "report": "matching",
  "total_checked": 3,
  "valid": 2,
  "invalid": 1,
  "truncated": false,
  "invalid_rows": [
    {
      "line": 3,
      "row": 2,
      "value": "FORBIDDEN",
      "message": "matches the pattern"
    }
  ]
}`;

test('regex-column-validate flags non-matching CSV cells with exact text output', async ({ page }) => {
  await page.goto('/tools/regex-column-validate/');
  await page.fill('#in-data', ZIP_CSV);
  await page.fill('#in-column', 'zip');
  await page.fill('#in-pattern', '\\d{5}');
  await page.check('#in-full_match');
  await page.selectOption('#in-report', 'non-matching');
  await page.selectOption('#in-output', 'text');

  await expect(page.locator('#tool-output')).toHaveText(TEXT_OUTPUT, { timeout: 15000 });
});

test('regex-column-validate supports inverted matching and JSON output', async ({ page }) => {
  await page.goto('/tools/regex-column-validate/');
  await page.fill('#in-data', 'status\nok\nFORBIDDEN\nfine');
  await page.fill('#in-column', 'status');
  await page.fill('#in-pattern', '[A-Z]+');
  await page.selectOption('#in-report', 'matching');
  await page.selectOption('#in-output', 'json');

  await expect(page.locator('#tool-output')).toHaveText(JSON_OUTPUT, { timeout: 15000 });
});

test('regex-column-validate deep-link pre-fills params and auto-runs', async ({ page }) => {
  const params = new URLSearchParams({
    data: ZIP_CSV,
    column: 'zip',
    pattern: '\\d{5}',
    full_match: 'true',
    ignore_case: 'false',
    report: 'non-matching',
    has_header: 'true',
    allow_blank: 'true',
    delimiter: 'auto',
    max_issues: '50',
    output: 'text',
  });

  await page.goto(`/tools/regex-column-validate/?${params.toString()}`);
  await expect(page.locator('#in-column')).toHaveValue('zip');
  await expect(page.locator('#in-full_match')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(TEXT_OUTPUT, { timeout: 15000 });
});

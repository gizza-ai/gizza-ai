import { test, expect } from './fixtures';

const CSV = 'name,status\nAda,active\nBo,activ\nCy,pending\nDe,active';

const TEXT_REPORT = `Column "status" (index 1) checked against allowed set: active, inactive, pending
Matching: case-sensitive, trimmed
Rows checked: 4
Valid: 3
Invalid: 1

Unexpected values:
  "activ" ×1

Invalid cells:
  row 2 (line 3): "activ" — not in the allowed set`;

async function addAllowedValues(page: any, values: string[]) {
  const input = page.locator('.tool-tags-search');
  for (const value of values) {
    await input.fill(value);
    await input.press('Enter');
  }
}

test('enum-domain-check flags unexpected CSV category values exactly', async ({ page }) => {
  await page.goto('/tools/enum-domain-check/');
  await page.fill('#in-data', CSV);
  await page.fill('#in-column', 'status');
  await addAllowedValues(page, ['active', 'inactive', 'pending']);

  await expect(page.locator('#tool-output')).toHaveText(TEXT_REPORT, { timeout: 15000 });
});

test('enum-domain-check deep-link emits JSON and honors enum output choice', async ({ page }) => {
  const params = new URLSearchParams({
    data: 'country\nUS\nusa\nUK\nUS',
    column: 'country',
    allowed: 'US,UK,CA',
    ignore_case: 'false',
    trim: 'true',
    has_header: 'true',
    allow_blank: 'true',
    delimiter: 'auto',
    max_issues: '50',
    output: 'json',
  });

  await page.goto(`/tools/enum-domain-check/?${params.toString()}`);
  await expect(page.locator('#in-output')).toHaveValue('json');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"invalid": 1', { timeout: 15000 });
  await expect(out).toContainText('"value": "usa"');
});

test('enum-domain-check non-default checkbox states affect validation', async ({ page }) => {
  await page.goto('/tools/enum-domain-check/');
  await page.fill('#in-data', 'id,status\n1,ACTIVE\n2,\n3,active');
  await page.fill('#in-column', 'status');
  await addAllowedValues(page, ['active']);
  await page.check('#in-ignore_case');
  await page.uncheck('#in-allow_blank');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Matching: case-insensitive, trimmed', { timeout: 15000 });
  await expect(out).toContainText('Invalid: 1');
  await expect(out).toContainText('blank value not allowed');
});

test('enum-domain-check delimiter enum and max boundary work', async ({ page }) => {
  await page.goto('/tools/enum-domain-check/');
  await page.fill('#in-data', 'id|status\n1|active\n2|bad\n3|nope');
  await page.fill('#in-column', 'status');
  await addAllowedValues(page, ['active']);
  await page.selectOption('#in-delimiter', 'pipe');
  await page.fill('#in-max_issues', '1');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Rows checked: 3', { timeout: 15000 });
  await expect(out).toContainText('Invalid: 2');
  await expect(out).toContainText('row 2 (line 3): "bad"');
  await expect(out).toContainText('1 more invalid cell(s) not shown');
});

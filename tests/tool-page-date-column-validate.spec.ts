import { test, expect } from './fixtures';

const BAD_CSV = `name,joined
Ada,2021-06-01
Bo,2021/06/02
Cy,2021-13-40`;

test('date-column-validate page reports invalid ISO dates by column name', async ({ page }) => {
  await page.goto('/tools/date-column-validate/');
  await page.fill('#in-data', BAD_CSV);
  await page.fill('#in-column', 'joined');

  const out = page.locator('#tool-output');
  await expect(out).toContainText(
    'Column "joined" (index 1) checked against %Y-%m-%d [ISO date (YYYY-MM-DD)]',
    { timeout: 15000 }
  );
  await expect(out).toContainText('Rows checked: 3');
  await expect(out).toContainText('Valid: 1');
  await expect(out).toContainText('Invalid: 2');
  await expect(out).toContainText('row 2 (line 3): "2021/06/02" — does not match %Y-%m-%d');
  // 2021-13-40 is impossible calendar (month 13, day 40), not just misshaped.
  await expect(out).toContainText('row 3 (line 4): "2021-13-40" — does not match %Y-%m-%d');
});

test('date-column-validate deep-link validates a custom strftime pattern', async ({ page }) => {
  const data = 'd\n01-Jun-2021\n2021-06-01';
  await page.goto(
    '/tools/date-column-validate/?' +
      new URLSearchParams({
        data,
        column: 'd',
        preset: 'custom',
        format: '%d-%b-%Y',
        has_header: 'true',
        allow_blank: 'true',
        delimiter: 'auto',
        max_issues: '50',
        output: 'text',
      }).toString()
  );

  await expect(page.locator('#in-data')).toHaveValue(data, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText(
    'Column "d" (index 0) checked against %d-%b-%Y [custom pattern %d-%b-%Y]',
    { timeout: 15000 }
  );
  await expect(out).toContainText('Valid: 1');
  await expect(out).toContainText('Invalid: 1');
  await expect(out).toContainText('row 2 (line 3): "2021-06-01" — does not match %d-%b-%Y');
});

test('date-column-validate JSON output is structured', async ({ page }) => {
  await page.goto('/tools/date-column-validate/');
  await page.fill('#in-data', 't\n2021-06-01T12:30:00Z\n2021-06-01 12:30');
  await page.fill('#in-column', 't');
  await page.selectOption('#in-preset', 'rfc3339');
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"valid": 1', { timeout: 15000 });
  await expect(out).toContainText('"invalid": 1');
  await expect(out).toContainText('"format": "RFC 3339"');
  await expect(out).toContainText('"value": "2021-06-01 12:30"');
});

test('date-column-validate us-date preset accepts MM/DD/YYYY only', async ({ page }) => {
  await page.goto('/tools/date-column-validate/');
  await page.fill('#in-data', 'd\n06/15/2021\n15/06/2021');
  await page.fill('#in-column', 'd');
  await page.selectOption('#in-preset', 'us-date');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Valid: 1', { timeout: 15000 });
  await expect(out).toContainText('Invalid: 1');
  await expect(out).toContainText('row 2 (line 3): "15/06/2021" — does not match %m/%d/%Y');
});

test('date-column-validate allow_blank checkbox off flags blank cells', async ({ page }) => {
  await page.goto('/tools/date-column-validate/');
  await page.fill('#in-data', 'a,d\nx,\ny,2021-06-02');
  await page.fill('#in-column', 'd');
  await page.uncheck('#in-allow_blank');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Invalid: 1', { timeout: 15000 });
  await expect(out).toContainText('(blank) — blank value not allowed');
});

test('date-column-validate caps the invalid list at max_issues', async ({ page }) => {
  await page.goto('/tools/date-column-validate/');
  await page.fill('#in-data', 'd\nx\ny\nz');
  await page.fill('#in-column', 'd');
  await page.fill('#in-max_issues', '2');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Invalid: 3', { timeout: 15000 });
  await expect(out).toContainText('… 1 more invalid value(s) not shown');
});

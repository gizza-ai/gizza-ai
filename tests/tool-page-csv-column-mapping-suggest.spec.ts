import { test, expect } from './fixtures';

const SOURCE = 'First Name,Email Address,Zip Code\nAda,a@example.com,02139\nBo,b@example.com,94107';
const TARGET = 'email,postal_code,first_name\na@example.com,02139,Ada\nb@example.com,94107,Bo';

async function setField(
  page: import('@playwright/test').Page,
  selector: string,
  value: string,
) {
  await page.locator(selector).evaluate((el, v) => {
    (el as HTMLTextAreaElement | HTMLInputElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('csv-column-mapping-suggest maps fuzzy headers and sample values as a table', async ({ page }) => {
  await page.goto('/tools/csv-column-mapping-suggest/');
  await setField(page, '#in-source', SOURCE);
  await setField(page, '#in-target', TARGET);

  const expected = [
    'Source column | Target column | Score | Reason',
    '--- | --- | ---: | ---',
    'Email Address | email | 0.720 | header 0.53, value 1.00',
    'First Name | first_name | 1.000 | header 1.00, value 1.00',
    'Zip Code | postal_code | 0.640 | header 0.40, value 1.00',
    '',
    'Unmapped source columns: (none)',
    'Unmapped target columns: (none)',
  ].join('\n');
  await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15000 });
});

test('csv-column-mapping-suggest supports JSON output and value-heavy matching', async ({ page }) => {
  await page.goto('/tools/csv-column-mapping-suggest/');
  await setField(page, '#in-source', 'customer_id\nA1\nB2\nC3');
  await setField(page, '#in-target', 'account\nA1\nB2\nC3');
  await page.fill('#in-header_weight', '0.2');
  await page.fill('#in-threshold', '0.5');
  await page.selectOption('#in-format', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"source_column": "customer_id"', { timeout: 15000 });
  await expect(out).toContainText('"target_column": "account"');
  await expect(out).toContainText('"value_score": 1.0');
});

test('csv-column-mapping-suggest deep-link pre-fills strict header-only csv output', async ({ page }) => {
  const qs = new URLSearchParams({
    source: 'First Name,Email Address\nAda,a@example.com',
    target: 'first_name,email\nAda,a@example.com',
    delimiter: 'comma',
    header: 'true',
    sample_rows: '0',
    header_weight: '1',
    threshold: '0.5',
    format: 'csv',
  });
  await page.goto(`/tools/csv-column-mapping-suggest/?${qs.toString()}`);

  await expect(page.locator('#in-source')).toHaveValue('First Name,Email Address\nAda,a@example.com', { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toContainText('source_column,target_column,score,header_score,value_score,reason', { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('First Name,first_name,1.000,1.000,0.000');
  await expect(page.locator('#tool-output')).toContainText('Email Address,email,0.533,0.533,0.000');
});

test('csv-column-mapping-suggest reports empty source errors', async ({ page }) => {
  await page.goto('/tools/csv-column-mapping-suggest/');
  await setField(page, '#in-source', '');
  await setField(page, '#in-target', TARGET);
  await expect(page.locator('#tool-output')).toContainText('source CSV is empty', { timeout: 15000 });
});

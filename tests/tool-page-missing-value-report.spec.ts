import { test, expect } from './fixtures';

const SAMPLE = `name,age,city
Alice,30,NYC
Bob,,LA
,25,
Carol,40,NYC`;

const EXPECTED = `column,missing,present,total,missing_percent
name,1,3,4,25%
age,1,3,4,25%
city,1,3,4,25%
Total rows: 4
Complete rows (no missing): 2 (50%)

Missingness patterns (present=1, missing=0):
count,name,age,city
2,1,1,1
1,1,0,1
1,0,1,0
`;

test('missing-value-report page reports counts, percentages, and patterns', async ({ page }) => {
  await page.goto('/tools/missing-value-report/');
  await page.fill('#in-input', SAMPLE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('column,missing,present,total,missing_percent', {
    timeout: 15_000,
  });
  expect(await out.textContent()).toBe(EXPECTED);
});

test('missing-value-report deep link supports TSV, original column order, and no pattern grid', async ({ page }) => {
  const data = 'id\tval\n1\t9\n2';
  const params = new URLSearchParams({
    input: data,
    delimiter: 'tab',
    na_values: '',
    sort: 'column',
    include_patterns: 'false',
    max_patterns: '10',
  });

  await page.goto(`/tools/missing-value-report/?${params.toString()}`);
  await expect(page.locator('#in-delimiter')).toHaveValue('tab', { timeout: 15_000 });
  await expect(page.locator('#in-sort')).toHaveValue('column');
  await expect(page.locator('#in-include_patterns')).not.toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('id\t0\t2\t2\t0%', { timeout: 15_000 });
  expect(await out.textContent()).toBe(
    'column\tmissing\tpresent\ttotal\tmissing_percent\nid\t0\t2\t2\t0%\nval\t1\t1\t2\t50%\nTotal rows: 2\nComplete rows (no missing): 1 (50%)\n',
  );
});

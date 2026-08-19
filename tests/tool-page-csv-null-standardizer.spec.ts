import { test, expect } from './fixtures';

const SAMPLE = 'id,score,notes\n1,42,ok\n2,NA,\n3,null,-\n4,n/a,NaN';
const STANDARDIZED_NULL = 'id,score,notes\n1,42,ok\n2,NULL,NULL\n3,NULL,NULL\n4,NULL,NULL\n';

test('csv-null-standardizer page standardizes common missing tokens exactly', async ({ page }) => {
  await page.goto('/tools/csv-null-standardizer/');
  await page.fill('#in-input', SAMPLE);
  await page.fill('#in-replace_with', 'NULL');
  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe(STANDARDIZED_NULL);
});

test('csv-null-standardizer page honors enum choices and non-default checkboxes', async ({ page }) => {
  await page.goto('/tools/csv-null-standardizer/');
  await page.fill('#in-input', 'a,b\nNA,na\n,NA');
  await page.fill('#in-na_tokens', 'NA');
  await page.fill('#in-replace_with', 'NULL');
  await page.check('#in-case_sensitive');
  await page.uncheck('#in-blank_is_missing');
  await page.selectOption('#in-quote_style', 'always');
  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe('"a","b"\n"NULL","na"\n"","NULL"\n');

  await page.selectOption('#in-quote_style', 'never');
  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe('a,b\nNULL,na\n,NULL\n');
});

test('csv-null-standardizer page supports auto-detected tab input and column scoping', async ({ page }) => {
  await page.goto('/tools/csv-null-standardizer/');
  await page.fill('#in-input', 'id\tval\tnotes\n1\tN/A\tN/A\n2\t9\t-');
  await page.fill('#in-delimiter', 'auto');
  await page.fill('#in-replace_with', 'NULL');
  await page.fill('#in-columns', 'val');
  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe('id\tval\tnotes\n1\tNULL\tN/A\n2\t9\t-\n');
});

test('csv-null-standardizer page honors query-param deep link', async ({ page }) => {
  await page.goto(
    '/tools/csv-null-standardizer/?input=' +
      encodeURIComponent(SAMPLE) +
      '&delimiter=comma&replace_with=NULL&blank_is_missing=true&case_sensitive=false&trim=true&header=true&quote_style=minimal',
  );
  await expect(page.locator('#in-input')).toHaveValue(SAMPLE, { timeout: 15000 });
  await expect(page.locator('#in-replace_with')).toHaveValue('NULL');
  await expect(page.locator('#in-quote_style')).toHaveValue('minimal');
  await expect
    .poll(async () => await page.locator('#tool-output').textContent(), { timeout: 15000 })
    .toBe(STANDARDIZED_NULL);
});

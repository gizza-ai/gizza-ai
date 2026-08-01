import { test, expect } from './fixtures';

test('query-result-formatter renders default JSON rows as exact Markdown', async ({ page }) => {
  await page.goto('/tools/query-result-formatter/');
  await page.fill('#in-data', '[{"id":1,"name":"Ada"},{"id":2,"name":"Linus"}]');
  await expect(page.locator('#tool-output')).toHaveText(
    '| id  | name  |\n| --- | ----- |\n| 1   | Ada   |\n| 2   | Linus |',
    { timeout: 15000 },
  );
});

test('query-result-formatter deep link parses SQL shell output as ASCII with null text', async ({ page }) => {
  const qs = new URLSearchParams({
    data: ' id | name\n----+------\n  1 | Ada\n  2 | \n(2 rows)',
    input_format: 'sql',
    format: 'ascii',
    null_text: 'NULL',
  });
  await page.goto('/tools/query-result-formatter/?' + qs.toString());
  await expect(page.locator('#in-input_format')).toHaveValue('sql', { timeout: 15000 });
  await expect(page.locator('#in-format')).toHaveValue('ascii');
  await expect(page.locator('#tool-output')).toHaveText(
    '+----+------+\n| id | name |\n+----+------+\n| 1  | Ada  |\n| 2  | NULL |\n+----+------+',
    { timeout: 15000 },
  );
});

test('query-result-formatter covers advertised formats, alignment, and header-off path', async ({ page }) => {
  await page.goto('/tools/query-result-formatter/');

  await page.fill('#in-data', 'name,score\nAda,10\nBo,2');
  await page.selectOption('#in-input_format', 'csv');
  await page.selectOption('#in-format', 'markdown');
  await page.selectOption('#in-align', 'right');
  await expect(page.locator('#tool-output')).toHaveText(
    '| name | score |\n| ---: | ----: |\n|  Ada |    10 |\n|   Bo |     2 |',
    { timeout: 15000 },
  );

  await page.fill('#in-data', '1\t2\n3\t4');
  await page.selectOption('#in-input_format', 'tsv');
  await page.selectOption('#in-format', 'ascii');
  await page.selectOption('#in-align', 'center');
  await page.uncheck('#in-header');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    '+----------+----------+\n| Column 1 | Column 2 |\n+----------+----------+\n|    1     |    2     |\n|    3     |    4     |\n+----------+----------+',
    { timeout: 15000 },
  );

  await page.fill('#in-data', '[{"a":1,"b":null},{"a":2}]');
  await page.selectOption('#in-input_format', 'json');
  await page.selectOption('#in-format', 'markdown');
  await page.selectOption('#in-align', 'left');
  await page.check('#in-header');
  await page.fill('#in-null_text', 'NULL');
  await expect(page.locator('#tool-output')).toHaveText(
    '| a   | b    |\n| --- | ---- |\n| 1   | NULL |\n| 2   | NULL |',
    { timeout: 15000 },
  );
});

import { test, expect } from './fixtures';

test('json-to-html-table renders default JSON rows as exact HTML', async ({ page }) => {
  await page.goto('/tools/json-to-html-table/');
  await page.fill('#in-json', '[{"id":1,"name":"Ada"},{"id":2,"name":"Linus"}]');
  await expect(page.locator('#tool-output')).toHaveText(
    '<table>\n  <thead>\n    <tr><th>id</th><th>name</th></tr>\n  </thead>\n  <tbody>\n    <tr><td>1</td><td>Ada</td></tr>\n    <tr><td>2</td><td>Linus</td></tr>\n  </tbody>\n</table>',
    { timeout: 15000 },
  );
});

test('json-to-html-table deep link renders flattened Markdown', async ({ page }) => {
  const qs = new URLSearchParams({
    json: '[{"user":{"id":1,"name":"Ada"}}]',
    format: 'markdown',
    nested: 'flatten',
  });
  await page.goto('/tools/json-to-html-table/?' + qs.toString());
  await expect(page.locator('#in-format')).toHaveValue('markdown', { timeout: 15000 });
  await expect(page.locator('#in-nested')).toHaveValue('flatten');
  await expect(page.locator('#tool-output')).toHaveText(
    '| user.id | user.name |\n| --- | --- |\n| 1 | Ada |',
    { timeout: 15000 },
  );
});

test('json-to-html-table covers markdown nulls, header-off arrays, and compact HTML controls', async ({ page }) => {
  await page.goto('/tools/json-to-html-table/');

  await page.fill('#in-json', '[{"a":1},{"a":2,"b":null}]');
  await page.selectOption('#in-format', 'markdown');
  await page.fill('#in-null_text', 'NULL');
  await expect(page.locator('#tool-output')).toHaveText(
    '| a | b |\n| --- | --- |\n| 1 | NULL |\n| 2 | NULL |',
    { timeout: 15000 },
  );

  await page.fill('#in-json', '[[1,2],[3,4]]');
  await page.selectOption('#in-format', 'markdown');
  await page.uncheck('#in-header');
  await expect(page.locator('#in-header')).not.toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(
    '| Column 1 | Column 2 |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |',
    { timeout: 15000 },
  );

  await page.fill('#in-json', '[{"name":"Ada","score":10}]');
  await page.selectOption('#in-format', 'html');
  await page.selectOption('#in-nested', 'json');
  await page.check('#in-header');
  await page.fill('#in-caption', 'Scores');
  await page.fill('#in-table_class', 'table table-striped');
  await page.uncheck('#in-pretty');
  await expect(page.locator('#tool-output')).toHaveText(
    '<table class="table table-striped"><caption>Scores</caption><thead><tr><th>name</th><th>score</th></tr></thead><tbody><tr><td>Ada</td><td>10</td></tr></tbody></table>',
    { timeout: 15000 },
  );
});

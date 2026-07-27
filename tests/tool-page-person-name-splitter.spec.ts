import { test, expect } from './fixtures';

const CSV = 'name,email\nDr. John Michael Smith Jr.,john@example.com\nLudwig van Beethoven,lvb@example.com\n"Smith, Jane Q",jane@example.com';

const APPEND_OUTPUT = `name,email,name_title,name_first,name_middle,name_last,name_suffix
Dr. John Michael Smith Jr.,john@example.com,Dr.,John,Michael,Smith,Jr.
Ludwig van Beethoven,lvb@example.com,,Ludwig,,van Beethoven,
"Smith, Jane Q",jane@example.com,,Jane,Q,Smith,`;

const REPLACE_OUTPUT = `name_title,name_first,name_middle,name_last,name_suffix,email
Dr.,John,Michael,Smith,Jr.,john@example.com
,Ludwig,,van Beethoven,,lvb@example.com
,Jane,Q,Smith,,jane@example.com`;

test('person-name-splitter appends parsed name components', async ({ page }) => {
  await page.goto('/tools/person-name-splitter/');
  await page.fill('#in-data', CSV);
  await page.fill('#in-name_column', 'name');
  await page.selectOption('#in-output', 'append');

  await expect(page.locator('#tool-output')).toHaveText(APPEND_OUTPUT, { timeout: 15000 });
});

test('person-name-splitter deep-link pre-fills params and replaces the source column', async ({ page }) => {
  const params = new URLSearchParams({
    data: CSV,
    name_column: 'name',
    output: 'replace',
    header: 'true',
    delimiter: 'comma',
    trim: 'true',
  });

  await page.goto(`/tools/person-name-splitter/?${params.toString()}`);
  await expect(page.locator('#in-name_column')).toHaveValue('name');
  await expect(page.locator('#in-output')).toHaveValue('replace');
  await expect(page.locator('#tool-output')).toHaveText(REPLACE_OUTPUT, { timeout: 15000 });
});

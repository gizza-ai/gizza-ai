import { test, expect } from './fixtures';

const peopleYaml = `- name: Ada
  address:
    city: London
    zip: N1
- name: Bo
  age: 40
`;

test('yaml-to-csv page flattens a list of records with unioned headers', async ({ page }) => {
  await page.goto('/tools/yaml-to-csv/');
  await page.fill('#in-data', peopleYaml);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('name,address.city,address.zip,age', { timeout: 15_000 });
  expect(await out.textContent()).toBe('name,address.city,address.zip,age\nAda,London,N1,\nBo,,,40\n');
});

test('yaml-to-csv page honors deep-linked delimiter, array mode and key column', async ({ page }) => {
  const data = `alice:
  tags: [admin, ops]
bob:
  tags: [dev]
`;
  const qs =
    '?data=' + encodeURIComponent(data) +
    '&delimiter=semicolon' +
    '&array_mode=columns' +
    '&key_column=id' +
    '&header=true' +
    '&quote_all=false';
  await page.goto('/tools/yaml-to-csv/' + qs);

  await expect(page.locator('#in-delimiter')).toHaveValue('semicolon', { timeout: 15_000 });
  await expect(page.locator('#in-array_mode')).toHaveValue('columns');
  await expect(page.locator('#in-key_column')).toHaveValue('id');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('id;tags.0;tags.1', { timeout: 15_000 });
  expect(await out.textContent()).toBe('id;tags.0;tags.1\nalice;admin;ops\nbob;dev;\n');
});

test('yaml-to-csv page covers no-header and quote-all checkbox states', async ({ page }) => {
  await page.goto('/tools/yaml-to-csv/');
  await page.fill('#in-data', '- a: 1\n  b: hi\n');
  await page.uncheck('#in-header');
  await page.check('#in-quote_all');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"1","hi"', { timeout: 15_000 });
  expect(await out.textContent()).toBe('"1","hi"\n');
});

test('yaml-to-csv page reports invalid yaml clearly', async ({ page }) => {
  await page.goto('/tools/yaml-to-csv/');
  await page.fill('#in-data', '- a: [1, 2\n');
  await expect(page.locator('#tool-output')).toContainText('YAML parse error', { timeout: 15_000 });
});

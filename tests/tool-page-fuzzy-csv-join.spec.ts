import { test, expect } from './fixtures';

const LEFT = `id,company
1,Acme Ltd
2,Globex Corporation
3,Initech`;

const RIGHT = `name,city
Acme Ltd.,Berlin
Globex Corp,Cairo
Umbrella,Delhi`;

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('fuzzy-csv-join page joins near company names with scores', async ({ page }) => {
  await page.goto('/tools/fuzzy-csv-join/');
  await page.fill('#in-left', LEFT);
  await page.fill('#in-right', RIGHT);
  await page.fill('#in-left_key', 'company');
  await page.fill('#in-right_key', 'name');
  await page.selectOption('#in-algorithm', 'jaro_winkler');
  await page.fill('#in-threshold', '85');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Globex Corp', { timeout: 15_000 });
  expect(await output(page)).toBe(`id,company,name,city,match_score
1,Acme Ltd,Acme Ltd.,Berlin,97.8
2,Globex Corporation,Globex Corp,Cairo,92.2`);
});

test('fuzzy-csv-join deep link pre-fills and keeps unmatched left rows', async ({ page }) => {
  const qs =
    '?left=' + encodeURIComponent(LEFT) +
    '&right=' + encodeURIComponent(RIGHT) +
    '&left_key=company' +
    '&right_key=name' +
    '&join_type=left' +
    '&threshold=85' +
    '&output=csv';
  await page.goto('/tools/fuzzy-csv-join/' + qs);

  await expect(page.locator('#in-left_key')).toHaveValue('company', { timeout: 15_000 });
  await expect(page.locator('#in-right_key')).toHaveValue('name');
  await expect(page.locator('#in-join_type')).toHaveValue('left');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Initech', { timeout: 15_000 });
  expect(await output(page)).toBe(`id,company,name,city,match_score
1,Acme Ltd,Acme Ltd.,Berlin,97.8
2,Globex Corporation,Globex Corp,Cairo,92.2
3,Initech,,,`);
});

test('fuzzy-csv-join page supports token sort and non-default checkbox', async ({ page }) => {
  await page.goto('/tools/fuzzy-csv-join/');
  await page.fill('#in-left', 'id,company\n1,Acme Limited');
  await page.fill('#in-right', 'name,city\nLimited Acme,Berlin');
  await page.fill('#in-left_key', 'company');
  await page.fill('#in-right_key', 'name');
  await page.selectOption('#in-algorithm', 'token_sort');
  await page.fill('#in-threshold', '100');
  await page.uncheck('#in-show_score');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Limited Acme', { timeout: 15_000 });
  expect(await output(page)).toBe(`id,company,name,city
1,Acme Limited,Limited Acme,Berlin`);
});

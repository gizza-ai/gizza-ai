import { test, expect } from './fixtures';

const CATALOG = `[
  {"name":"Aero Jacket","brand":"Northwind","price":180,"tags":["outdoor","sale"],"in_stock":true},
  {"name":"Basalt Boots","brand":"Northwind","price":120,"tags":["outdoor"],"in_stock":false},
  {"name":"Cirrus Cap","brand":"Trailhead","price":25,"tags":["sale"],"in_stock":true},
  {"name":"Delta Pack","brand":"Summit","price":95,"tags":["outdoor","travel"],"in_stock":true}
]`;

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('faceted-filter page renders exact facet summary with array counts', async ({ page }) => {
  await page.goto('/tools/faceted-filter/');
  await page.fill('#in-data', CATALOG);
  await page.fill('#in-facets', 'brand, tags');
  await page.selectOption('#in-output', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('4 records matched', { timeout: 15_000 });
  expect(await output(page)).toBe(`4 records matched — page 1 of 1 (4 shown, 10 per page)

brand (3 distinct values)
  Northwind  2
  Summit     1
  Trailhead  1

tags (3 distinct values)
  outdoor  3
  sale     2
  travel   1`);
});

test('faceted-filter deep link pre-fills filters and returns real facets', async ({ page }) => {
  const qs =
    '?data=' + encodeURIComponent(CATALOG) +
    '&filters=' + encodeURIComponent('tags contains outdoor and in_stock') +
    '&facets=' + encodeURIComponent('brand') +
    '&sort=' + encodeURIComponent('price:desc') +
    '&per_page=2' +
    '&output=facets';
  await page.goto('/tools/faceted-filter/' + qs);

  await expect(page.locator('#in-filters')).toHaveValue('tags contains outdoor and in_stock', { timeout: 15_000 });
  await expect(page.locator('#in-sort')).toHaveValue('price:desc');
  await expect(page.locator('#in-per_page')).toHaveValue('2');
  await expect(page.locator('#in-output')).toHaveValue('facets');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Northwind', { timeout: 15_000 });
  expect(JSON.parse(await output(page))).toEqual([
    {
      field: 'brand',
      distinct: 2,
      values: [
        { value: 'Northwind', count: 1 },
        { value: 'Summit', count: 1 },
      ],
    },
  ]);
});

test('faceted-filter page supports non-default checkbox and value sorting', async ({ page }) => {
  await page.goto('/tools/faceted-filter/');
  await page.fill('#in-data', CATALOG);
  await page.fill('#in-facets', 'price');
  await page.check('#in-facet_stats');
  await page.selectOption('#in-facet_sort', 'value_asc');
  await page.selectOption('#in-output', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('min 25 · max 180 · avg 105 · sum 420', { timeout: 15_000 });
  expect(await output(page)).toContain(`price (4 distinct values)
  25   1
  95   1
  120  1
  180  1`);
});

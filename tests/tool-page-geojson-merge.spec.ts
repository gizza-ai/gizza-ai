import { test, expect } from './fixtures';

const INPUT = [
  '{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A"},"geometry":{"type":"Point","coordinates":[0,0]}}]}',
  '{"type":"Point","coordinates":[1,1]}',
].join('\n');

async function setGeojson(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-geojson').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('geojson-merge combines FeatureCollections and bare geometries with exact output', async ({ page }) => {
  await page.goto('/tools/geojson-merge/');
  await setGeojson(page, INPUT);
  await page.locator('#in-indent').fill('0');

  const expected = '{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A"},"geometry":{"type":"Point","coordinates":[0,0]}},{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[1,1]}}]}';
  await expect(page.locator('#tool-output')).toHaveText(expected, { timeout: 15_000 });
});

test('geojson-merge deep-link renumbers and tags source property', async ({ page }) => {
  const qs = new URLSearchParams({
    geojson: INPUT,
    indent: '0',
    precision: '-1',
    renumber: 'true',
    source_property: 'source',
  });
  await page.goto(`/tools/geojson-merge/?${qs.toString()}`);

  await expect(page.locator('#in-geojson')).toHaveValue(INPUT);
  await expect(page.locator('#in-indent')).toHaveValue('0');
  await expect(page.locator('#in-renumber')).toBeChecked();
  await expect(page.locator('#in-source_property')).toHaveValue('source');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"id":0', { timeout: 15_000 });
  await expect(out).toContainText('"source":1');
  await expect(out).toContainText('"source":2');
});

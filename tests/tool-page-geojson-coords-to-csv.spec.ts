import { test, expect } from './fixtures';

const lineFeature =
  '{"type":"Feature","properties":{"name":"Ridge trail"},"geometry":{"type":"LineString","coordinates":[[-105.1,40.1,2410],[-105.2,40.2,2530]]}}';

const polygon =
  '{"type":"Polygon","coordinates":[[[-105.0,40.0],[-104.9,40.0],[-104.9,40.1],[-105.0,40.0]]]}';

const points =
  '{"type":"FeatureCollection","features":[{"type":"Feature","id":"a","properties":{"name":"Depot"},"geometry":{"type":"Point","coordinates":[-105.27055,40.01499]}},{"type":"Feature","id":"b","properties":{"name":"Yard"},"geometry":{"type":"Point","coordinates":[-105.08442,40.58526]}}]}';

test('geojson-coords-to-csv page extracts every line coordinate with elevation', async ({ page }) => {
  await page.goto('/tools/geojson-coords-to-csv/');
  await page.fill('#in-geojson', lineFeature);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('longitude,latitude,elevation', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`longitude,latitude,elevation
-105.1,40.1,2410
-105.2,40.2,2530`);
});

test('geojson-coords-to-csv page deep-link swaps axis order and appends properties', async ({ page }) => {
  const qs =
    '?geojson=' +
    encodeURIComponent(points) +
    '&order=latlon' +
    '&columns=coords' +
    '&shapes=all' +
    '&elevation=never' +
    '&precision=5' +
    '&dedupe=none' +
    '&properties=true' +
    '&delimiter=comma' +
    '&header=true';
  await page.goto('/tools/geojson-coords-to-csv/' + qs);

  await expect(page.locator('#in-order')).toHaveValue('latlon', { timeout: 15_000 });
  await expect(page.locator('#in-properties')).toBeChecked();

  const out = page.locator('#tool-output');
  await expect(out).toContainText('latitude,longitude,name', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`latitude,longitude,name
40.01499,-105.27055,Depot
40.58526,-105.08442,Yard`);
});

test('geojson-coords-to-csv page drops polygon closing coordinate and adds indexes', async ({ page }) => {
  await page.goto('/tools/geojson-coords-to-csv/');
  await page.fill('#in-geojson', polygon);
  await page.selectOption('#in-columns', 'indexed');
  await page.selectOption('#in-dedupe', 'ring-close');
  await page.selectOption('#in-elevation', 'never');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('index,longitude,latitude', { timeout: 15_000 });
  expect(await out.textContent()).toBe(`index,longitude,latitude
1,-105.0,40.0
2,-104.9,40.0
3,-104.9,40.1`);
});

test('geojson-coords-to-csv page supports delimiter and no-header advertised values', async ({ page }) => {
  await page.goto('/tools/geojson-coords-to-csv/');
  await page.fill('#in-geojson', '{"type":"Point","coordinates":[1,2]}');
  await page.selectOption('#in-delimiter', 'semicolon');
  await page.locator('#in-header').uncheck();

  await expect(page.locator('#tool-output')).toHaveText('1;2', { timeout: 15_000 });
});

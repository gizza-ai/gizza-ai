import { test, expect } from './fixtures';

const fc = JSON.stringify({
  type: 'FeatureCollection',
  features: [
    {
      type: 'Feature',
      properties: { name: 'Alpha', pop: 1200 },
      geometry: { type: 'Point', coordinates: [-105, 40] },
    },
    {
      type: 'Feature',
      properties: { name: 'Beta', pop: 800 },
      geometry: { type: 'Point', coordinates: [-106.5, 41.25] },
    },
  ],
});

test('geojson-to-csv page: FeatureCollection to WKT CSV', async ({ page }) => {
  await page.goto('/tools/geojson-to-csv/');
  await page.fill('#in-geojson', fc);
  await expect(page.locator('#tool-output')).toHaveText(
    'name,pop,geometry\nAlpha,1200,POINT (-105 40)\nBeta,800,POINT (-106.5 41.25)',
    { timeout: 15000 },
  );
});

test('geojson-to-csv page: lonlat geometry and no header', async ({ page }) => {
  await page.goto('/tools/geojson-to-csv/');
  await page.fill('#in-geojson', fc);
  await page.selectOption('#in-geometry', 'lonlat');
  await page.uncheck('#in-header');
  await expect(page.locator('#tool-output')).toHaveText(
    'Alpha,1200,-105,40\nBeta,800,-106.5,41.25',
    { timeout: 15000 },
  );
});

test('geojson-to-csv page: flatten nested properties', async ({ page }) => {
  await page.goto('/tools/geojson-to-csv/');
  await page.fill(
    '#in-geojson',
    '{"type":"Feature","properties":{"name":"A","address":{"city":"Denver"}},"geometry":null}',
  );
  await page.selectOption('#in-geometry', 'none');
  await page.selectOption('#in-nested', 'flatten');
  await expect(page.locator('#tool-output')).toHaveText('name,address.city\nA,Denver', {
    timeout: 15000,
  });
});

test('geojson-to-csv page: query-param deep-link prefills and runs', async ({ page }) => {
  await page.goto(
    '/tools/geojson-to-csv/?geojson=' +
      encodeURIComponent(fc) +
      '&geometry=both&nested=json&delimiter=pipe&header=true',
  );
  await expect(page.locator('#in-geojson')).toHaveValue(fc, { timeout: 15000 });
  await expect(page.locator('#in-geometry')).toHaveValue('both');
  await expect(page.locator('#in-delimiter')).toHaveValue('pipe');
  await expect(page.locator('#tool-output')).toHaveText(
    'name|pop|longitude|latitude|geometry\nAlpha|1200|-105|40|POINT (-105 40)\nBeta|800|-106.5|41.25|POINT (-106.5 41.25)',
    { timeout: 15000 },
  );
});

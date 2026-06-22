import { test, expect } from './fixtures';

// /tools/geojson-to-svg/ renders GeoJSON into an SVG map in-browser (pure wasm).
test('geojson-to-svg renders a polygon FeatureCollection into SVG', async ({ page }) => {
  await page.goto('/tools/geojson-to-svg/');
  await page.fill(
    '#in-geojson',
    '{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]]]},"properties":{}}]}'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<svg', { timeout: 15000 });
  await expect(out).toContainText('</svg>');
  await expect(out).toContainText('<path');
});

test('geojson-to-svg renders a bare Point as a circle', async ({ page }) => {
  await page.goto('/tools/geojson-to-svg/');
  await page.fill('#in-geojson', '{"type":"Point","coordinates":[5,5]}');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<circle', { timeout: 15000 });
});

test('geojson-to-svg honours a custom fill colour', async ({ page }) => {
  await page.goto('/tools/geojson-to-svg/');
  await page.fill('#in-fill', '#ff0000');
  await page.fill(
    '#in-geojson',
    '{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,0]]]}'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('#ff0000', { timeout: 15000 });
});

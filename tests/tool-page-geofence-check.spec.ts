import { test, expect } from './fixtures';

const polygon = '0,0\n0,10\n10,10\n10,0';
const points = '5,5,center\n10,5,edge\n20,20,out';

// /tools/geofence-check/ checks many latitude/longitude points against one polygon in-browser.
test('geofence-check renders exact text result for a square polygon', async ({ page }) => {
  await page.goto('/tools/geofence-check/');
  await page.fill('#in-polygon', polygon);
  await page.fill('#in-points', points);
  await page.selectOption('#in-coord_order', 'lat_lon');
  await page.selectOption('#in-boundary', 'inside');
  await page.selectOption('#in-output', 'text');

  await expect(page.locator('#tool-output')).toHaveText(
    '3 points: 2 inside, 1 outside\n#1  5, 5 (center)  inside\n#2  10, 5 (edge)  inside\n#3  20, 20 (out)  outside',
    { timeout: 15000 },
  );
});

test('geofence-check deep-link renders CSV with boundary status', async ({ page }) => {
  await page.goto(
    '/tools/geofence-check/?polygon=' +
      encodeURIComponent(polygon) +
      '&points=' +
      encodeURIComponent(points) +
      '&coord_order=lat_lon&boundary=boundary&output=csv',
  );
  await expect(page.locator('#in-polygon')).toHaveValue(polygon, { timeout: 15000 });
  await expect(page.locator('#in-boundary')).toHaveValue('boundary');
  await expect(page.locator('#tool-output')).toHaveText(
    'point,latitude,longitude,label,status\n1,5,5,center,inside\n2,10,5,edge,boundary\n3,20,20,out,outside',
    { timeout: 15000 },
  );
});

test('geofence-check supports GeoJSON polygon and point features', async ({ page }) => {
  const geoPolygon = '{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]]]}';
  const geoPoints = '{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"inside"},"geometry":{"type":"Point","coordinates":[5,5]}},{"type":"Feature","properties":{"name":"outside"},"geometry":{"type":"Point","coordinates":[20,20]}}]}';
  await page.goto('/tools/geofence-check/');
  await page.fill('#in-polygon', geoPolygon);
  await page.fill('#in-points', geoPoints);
  await page.selectOption('#in-boundary', 'outside');
  await page.selectOption('#in-output', 'json');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"inside": 1', { timeout: 15000 });
  await expect(out).toContainText('"outside": 1');
  await expect(out).toContainText('"label": "inside"');
});

test('geofence-check reports invalid latitude with swap hint', async ({ page }) => {
  await page.goto('/tools/geofence-check/');
  await page.fill('#in-polygon', polygon);
  await page.fill('#in-points', '200,5,bad');
  await expect(page.locator('#tool-output')).toContainText("set coord_order to 'lon_lat'", {
    timeout: 15000,
  });
});

import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const SIMPLE = '{"type":"Topology","objects":{"line":{"type":"LineString","arcs":[0],"properties":{"name":"A"}}},"arcs":[[[0,0],[1,1]]]}';
const TRANSFORMED_POINT = '{"type":"Topology","transform":{"scale":[0.1,0.1],"translate":[10,20]},"objects":{"pts":{"type":"MultiPoint","coordinates":[[3,3],[7,9]]}},"arcs":[]}';

test('topojson-to-geojson page expands an arc to exact GeoJSON', async ({ page }) => {
  await page.goto('/tools/topojson-to-geojson/');
  await page.fill('#in-topojson', SIMPLE);
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toContainText('FeatureCollection', { timeout: 15_000 });
  expect(await output(page)).toBe('{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A"},"geometry":{"type":"LineString","coordinates":[[0.0,0.0],[1.0,1.0]]}}]}');
});

test('topojson-to-geojson deep link selects geometry collection and rounded transform', async ({ page }) => {
  const qs = new URLSearchParams({
    topojson: TRANSFORMED_POINT,
    object: 'pts',
    output: 'geometry-collection',
    precision: '1',
    indent: '0',
  });
  await page.goto(`/tools/topojson-to-geojson/?${qs.toString()}`);

  await expect(page.locator('#in-output')).toHaveValue('geometry-collection', { timeout: 15_000 });
  expect(await output(page)).toBe('{"type":"GeometryCollection","geometries":[{"type":"MultiPoint","coordinates":[[10.3,20.3],[10.7,20.9]]}]}');
});

test('topojson-to-geojson page can include a bbox and report unknown objects', async ({ page }) => {
  await page.goto('/tools/topojson-to-geojson/');
  await page.fill('#in-topojson', SIMPLE);
  await page.fill('#in-object', 'line');
  await page.check('#in-include_bbox');
  await page.fill('#in-indent', '0');

  await expect(page.locator('#tool-output')).toContainText('"bbox":[0.0,0.0,1.0,1.0]', { timeout: 15_000 });

  await page.fill('#in-object', 'countries');
  await expect(page.locator('#tool-output')).toContainText('no object named "countries"', { timeout: 15_000 });
});

import { test, expect } from './fixtures';

const tool = '/tools/csv-to-geojson/';
const pointsCsv = 'name,lat,lon\nDenver,39.7392,-104.9903\nBoulder,40.0150,-105.2705';
const expectedPoints = '{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[-104.9903,39.7392]},"properties":{"name":"Denver"}},{"type":"Feature","geometry":{"type":"Point","coordinates":[-105.2705,40.015]},"properties":{"name":"Boulder"}}]}';
const expectedLine = '{"type":"Feature","bbox":[-105.2705,39.7392,-104.9903,40.015],"geometry":{"type":"LineString","coordinates":[[-104.9903,39.7392],[-105.2705,40.015]]},"properties":{}}';

async function setTextarea(locator, value: string) {
  await locator.evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v as string;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

async function runWasm(
  page,
  input = pointsCsv,
  lat = '',
  lon = '',
  elevation = '',
  delimiter = 'auto',
  shape = 'points',
  types = 'infer',
  precision = '4',
  invalid = 'skip',
  bbox = 'false',
  pretty = 'false',
): Promise<string> {
  return await page.evaluate(
    async ({ input, lat, lon, elevation, delimiter, shape, types, precision, invalid, bbox, pretty }) => {
      const mod = await import('/tools/csv-to-geojson/gizza_ai_csv_to_geojson_web.js');
      await mod.default('/tools/csv-to-geojson/gizza_ai_csv_to_geojson_web_bg.wasm');
      return mod.run(input, lat, lon, elevation, delimiter, shape, types, precision, invalid, bbox, pretty);
    },
    { input, lat, lon, elevation, delimiter, shape, types, precision, invalid, bbox, pretty },
  );
}

test('csv-to-geojson page renders exact point FeatureCollection', async ({ page }) => {
  await page.goto(tool);
  await setTextarea(page.locator('#in-input'), pointsCsv);
  await page.fill('#in-precision', '4');
  await page.uncheck('#in-pretty');
  await expect(page.locator('#tool-output')).toHaveText(expectedPoints, { timeout: 15_000 });
});

test('csv-to-geojson deep link pre-fills and runs a LineString with bbox', async ({ page }) => {
  const qs = new URLSearchParams({
    input: pointsCsv,
    lat: '',
    lon: '',
    elevation: '-',
    delimiter: 'auto',
    shape: 'line',
    types: 'infer',
    precision: '4',
    invalid: 'error',
    bbox: 'true',
    pretty: 'false',
  });
  await page.goto(`${tool}?${qs.toString()}`);
  await expect(page.locator('#in-input')).toHaveValue(pointsCsv, { timeout: 15_000 });
  await expect(page.locator('#in-shape')).toHaveValue('line');
  await expect(page.locator('#in-bbox')).toBeChecked();
  await expect(page.locator('#tool-output')).toHaveText(expectedLine, { timeout: 15_000 });
});

test('csv-to-geojson wasm covers advertised shapes, delimiters, types and invalid-row modes', async ({ page }) => {
  await page.goto(tool);

  expect(await runWasm(page)).toBe(expectedPoints);

  const semicolon = await runWasm(page, 'name;lat;lon\nOslo;59,91;10,75', 'lat', 'lon', '-', 'semicolon');
  expect(JSON.parse(semicolon).features[0].geometry.coordinates).toEqual([10.75, 59.91]);

  const jsonRows = await runWasm(page, '[{"name":"A","lat":40,"lon":-105},{"name":"B","lat":41,"lon":-106}]', '', '', '', 'auto', 'points', 'infer', '0', 'skip', 'false', 'false');
  expect(JSON.parse(jsonRows).features[1].properties.name).toBe('B');

  const stringTypes = await runWasm(page, 'lat,lon,pop\n40,-105,1200', '', '', '-', 'auto', 'points', 'string');
  expect(JSON.parse(stringTypes).features[0].properties.pop).toBe('1200');

  const nullRows = await runWasm(page, 'name,lat,lon\nValid,40,-105\nMissing,,', '', '', '-', 'auto', 'points', 'infer', '0', 'null', 'false', 'false');
  expect(JSON.parse(nullRows).features[1]).toMatchObject({ geometry: null, properties: { name: 'Missing' } });

  const polygon = await runWasm(page, 'lat,lon\n0,0\n0,1\n1,1\n1,0', '', '', '-', 'auto', 'polygon', 'infer', '0', 'error', 'true', 'false');
  const poly = JSON.parse(polygon);
  expect(poly.geometry.type).toBe('Polygon');
  expect(poly.bbox).toEqual([0, 0, 1, 1]);

  await expect(runWasm(page, 'lat,lon\n95,-105', '', '', '', 'auto', 'points', 'infer', '0', 'error')).rejects.toThrow(/latitude 95 is outside/);
  await expect(runWasm(page, pointsCsv, '', '', '', 'auto', 'route')).rejects.toThrow(/unknown shape/);
  await expect(runWasm(page, pointsCsv, '', '', '', 'auto', 'points', 'infer', '16')).rejects.toThrow(/precision must be between 0 and 15/);
});

test('csv-to-geojson ships example chips', async ({ page }) => {
  await page.goto(tool);
  await expect(page.locator('.tool-example-chip')).toHaveCount(6);
  await expect(page.locator('.tool-example-chip')).toContainText([
    'Two point features',
    'LineString route with bbox',
    'Semicolon + comma decimals',
    'JSON rows to points',
    'Keep invalid rows as null geometry',
    'Polygon from ordered vertices',
  ]);
});

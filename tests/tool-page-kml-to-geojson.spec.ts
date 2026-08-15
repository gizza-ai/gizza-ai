import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const SIMPLE_KML = '<kml xmlns="http://www.opengis.net/kml/2.2"><Document><Folder><name>Trails</name>' +
  '<Placemark><name>Trailhead</name><description>Parking lot</description>' +
  '<Point><coordinates>-122.0841234,37.4212345,15</coordinates></Point></Placemark>' +
  '</Folder></Document></kml>';

const STYLED_KML = '<kml xmlns="http://www.opengis.net/kml/2.2"><Document>' +
  '<Style id="route"><LineStyle><color>ff0000ff</color><width>4</width></LineStyle></Style>' +
  '<Placemark><name>River Loop</name><styleUrl>#route</styleUrl>' +
  '<LineString><coordinates>5.1,52.1 5.102,52.101 5.104,52.103</coordinates></LineString>' +
  '</Placemark></Document></kml>';

const GEOJSON = '{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"Camp","folder":"Trails/Day 1","marker-color":"#0000ff"},"geometry":{"type":"Point","coordinates":[5.1,52.1,10]}}]}';

test('converts KML folder and placemark to exact GeoJSON output', async ({ page }) => {
  await page.goto('/tools/kml-to-geojson/');
  await page.fill('#in-input', SIMPLE_KML);
  await page.fill('#in-precision', '6');

  await expect(page.locator('#tool-output')).toContainText('"FeatureCollection"', { timeout: 15_000 });
  expect(await output(page)).toBe(
    '{\n' +
      '  "type": "FeatureCollection",\n' +
      '  "features": [\n' +
      '    {\n' +
      '      "type": "Feature",\n' +
      '      "geometry": {\n' +
      '        "type": "Point",\n' +
      '        "coordinates": [\n' +
      '          -122.084123,\n' +
      '          37.421235,\n' +
      '          15.0\n' +
      '        ]\n' +
      '      },\n' +
      '      "properties": {\n' +
      '        "name": "Trailhead",\n' +
      '        "description": "Parking lot",\n' +
      '        "folder": "Trails"\n' +
      '      }\n' +
      '    }\n' +
      '  ]\n' +
      '}',
  );
});

test('preserves styles by default and can disable styles with a non-default checkbox state', async ({ page }) => {
  await page.goto('/tools/kml-to-geojson/');
  await expect(page.locator('#in-include_styles')).toBeChecked();
  await page.fill('#in-input', STYLED_KML);
  await page.fill('#in-precision', '5');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('"stroke": "#ff0000"', { timeout: 15_000 });
  await expect(out).toContainText('"stroke-width": 4.0');

  await page.uncheck('#in-include_styles');
  await expect(out).toContainText('"LineString"', { timeout: 15_000 });
  await expect(out).not.toContainText('"stroke"');
});

test('converts GeoJSON back to KML with enum controls and document metadata', async ({ page }) => {
  await page.goto('/tools/kml-to-geojson/');
  await page.selectOption('#in-output_format', 'kml');
  await page.fill('#in-input', GEOJSON);
  await page.fill('#in-document_name', 'Weekend Trip');
  await page.selectOption('#in-altitude_mode', 'absolute');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<name>Weekend Trip</name>', { timeout: 15_000 });
  await expect(out).toContainText('<name>Trails</name>');
  await expect(out).toContainText('<name>Day 1</name>');
  await expect(out).toContainText('<altitudeMode>absolute</altitudeMode>');
  await expect(out).toContainText('<coordinates>5.1,52.1,10</coordinates>');
});

test('deep link pre-fills KML and precision and auto-runs', async ({ page }) => {
  const qs = new URLSearchParams({
    input: SIMPLE_KML,
    output_format: 'geojson',
    include_styles: 'true',
    include_folders: 'true',
    precision: '0',
  });
  await page.goto(`/tools/kml-to-geojson/?${qs.toString()}`);

  await expect(page.locator('#in-precision')).toHaveValue('0', { timeout: 15_000 });
  expect(await output(page)).toContain('"coordinates": [\n          -122.0,\n          37.0,\n          15.0\n        ]');
});

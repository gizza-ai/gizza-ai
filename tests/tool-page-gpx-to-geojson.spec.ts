import { test, expect } from './fixtures';

// /tools/gpx-to-geojson/ converts GPX/KML GPS tracks & waypoints into
// GeoJSON, and GeoJSON back into GPX (pure wasm, no ffmpeg).

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('converts a GPX track with elevation and timestamps to a GeoJSON LineString (exact output)', async ({ page }) => {
  await page.goto('/tools/gpx-to-geojson/');
  await page.fill(
    '#in-input',
    '<?xml version="1.0"?>\n<gpx version="1.1"><trk><name>Morning Run</name><trkseg>\n' +
      '<trkpt lat="52.100" lon="5.100"><ele>10</ele><time>2026-07-01T08:00:00Z</time></trkpt>\n' +
      '<trkpt lat="52.101" lon="5.102"><ele>12</ele><time>2026-07-01T08:05:00Z</time></trkpt>\n' +
      '</trkseg></trk></gpx>',
  );
  await expect(page.locator('#tool-output')).toContainText('"LineString"', { timeout: 15000 });
  expect(await output(page)).toBe(
    '{\n' +
      '  "type": "FeatureCollection",\n' +
      '  "features": [\n' +
      '    {\n' +
      '      "type": "Feature",\n' +
      '      "geometry": {\n' +
      '        "type": "LineString",\n' +
      '        "coordinates": [\n' +
      '          [\n' +
      '            5.1,\n' +
      '            52.1,\n' +
      '            10.0\n' +
      '          ],\n' +
      '          [\n' +
      '            5.102,\n' +
      '            52.101,\n' +
      '            12.0\n' +
      '          ]\n' +
      '        ]\n' +
      '      },\n' +
      '      "properties": {\n' +
      '        "name": "Morning Run",\n' +
      '        "coordTimes": [\n' +
      '          "2026-07-01T08:00:00Z",\n' +
      '          "2026-07-01T08:05:00Z"\n' +
      '        ]\n' +
      '      }\n' +
      '    }\n' +
      '  ]\n' +
      '}',
  );
});

test('converts a GPX waypoint to a GeoJSON Point', async ({ page }) => {
  await page.goto('/tools/gpx-to-geojson/');
  await page.fill('#in-input', '<gpx version="1.1"><wpt lat="10" lon="20"><name>Camp</name><ele>100</ele></wpt></gpx>');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"Point"', { timeout: 15000 });
  await expect(out).toContainText('"coordinates": [\n          20.0,\n          10.0,\n          100.0\n        ]');
  await expect(out).toContainText('"name": "Camp"');
});

test('resolves a KML styleUrl through a shared Style into stroke properties (default checked)', async ({ page }) => {
  await page.goto('/tools/gpx-to-geojson/');
  // include_styles defaults to checked (Param::boolean(...).default(true)).
  await expect(page.locator('#in-include_styles')).toBeChecked();
  await page.fill(
    '#in-input',
    '<kml><Document><Style id="s1"><LineStyle><color>ff00ff00</color><width>2</width></LineStyle></Style>' +
      '<Placemark><styleUrl>#s1</styleUrl><LineString><coordinates>0,0 1,1</coordinates></LineString></Placemark>' +
      '</Document></kml>',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"stroke": "#00ff00"', { timeout: 15000 });
  await expect(out).toContainText('"stroke-width": 2.0');
});

test('omits KML style properties when include_styles is unchecked (non-default checkbox state)', async ({ page }) => {
  await page.goto('/tools/gpx-to-geojson/');
  await page.fill(
    '#in-input',
    '<kml><Placemark><Style><LineStyle><color>ff0000ff</color></LineStyle></Style>' +
      '<LineString><coordinates>0,0 1,1</coordinates></LineString></Placemark></kml>',
  );
  await page.uncheck('#in-include_styles');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"LineString"', { timeout: 15000 });
  await expect(out).not.toContainText('"stroke"');
});

test('converts GeoJSON back to GPX when output_format is switched to gpx (enum select)', async ({ page }) => {
  await page.goto('/tools/gpx-to-geojson/');
  await page.selectOption('#in-output_format', 'gpx');
  await page.fill(
    '#in-input',
    '{"type":"FeatureCollection","features":[' +
      '{"type":"Feature","properties":{"name":"Camp"},"geometry":{"type":"Point","coordinates":[5.1,52.1,10]}},' +
      '{"type":"Feature","properties":{"name":"Loop"},"geometry":{"type":"LineString","coordinates":[[5.1,52.1],[5.102,52.101]]}}' +
      ']}',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<wpt lat="52.1" lon="5.1">', { timeout: 15000 });
  await expect(out).toContainText('<name>Camp</name>');
  await expect(out).toContainText('<trk>');
  await expect(out).toContainText('<name>Loop</name>');
});

test('shows a helpful error when output_format=gpx is used with XML input', async ({ page }) => {
  await page.goto('/tools/gpx-to-geojson/');
  await page.selectOption('#in-output_format', 'gpx');
  await page.fill('#in-input', '<gpx version="1.1"><wpt lat="1" lon="1"/></gpx>');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('output_format="gpx"', { timeout: 15000 });
});

test('pre-fills and auto-runs from query params (deep link)', async ({ page }) => {
  const gpx = '<gpx version="1.1"><rte><name>Route A</name><rtept lat="1" lon="1"/><rtept lat="2" lon="2"/></rte></gpx>';
  await page.goto('/tools/gpx-to-geojson/?input=' + encodeURIComponent(gpx) + '&output_format=geojson');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"LineString"', { timeout: 15000 });
  await expect(out).toContainText('"name": "Route A"');
});

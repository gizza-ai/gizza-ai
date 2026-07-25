import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const TRACK =
  '<?xml version="1.0"?>\n' +
  '<gpx version="1.1"><metadata><name>Trip</name></metadata>' +
  '<trk><name>Morning Run</name><trkseg>' +
  '<trkpt lat="52.100" lon="5.100"><ele>10</ele><time>2026-07-01T08:00:00Z</time></trkpt>' +
  '<trkpt lat="52.101" lon="5.102"><ele>12</ele><time>2026-07-01T08:05:00Z</time></trkpt>' +
  '</trkseg></trk></gpx>';

const WAYPOINT = '<gpx version="1.1"><wpt lat="1" lon="2"><name>Camp</name></wpt></gpx>';

test('gpx-to-kml page: converts a GPX track to exact KML with default styling', async ({ page }) => {
  await page.goto('/tools/gpx-to-kml/');
  await page.fill('#in-gpx', TRACK);
  await expect(page.locator('#tool-output')).toContainText('<LineString>', { timeout: 15000 });
  expect(await outputText(page)).toBe(
    '<?xml version="1.0" encoding="UTF-8"?>\n' +
      '<kml xmlns="http://www.opengis.net/kml/2.2">\n' +
      '  <Document>\n' +
      '    <name>Trip</name>\n' +
      '    <Style id="lineStyle">\n' +
      '      <LineStyle>\n' +
      '        <color>cc4444ef</color>\n' +
      '        <width>4</width>\n' +
      '      </LineStyle>\n' +
      '    </Style>\n' +
      '    <Style id="waypointStyle">\n' +
      '      <IconStyle>\n' +
      '        <color>fff6823b</color>\n' +
      '      </IconStyle>\n' +
      '    </Style>\n' +
      '    <Placemark>\n' +
      '      <name>Morning Run</name>\n' +
      '      <styleUrl>#lineStyle</styleUrl>\n' +
      '      <TimeSpan>\n' +
      '        <begin>2026-07-01T08:00:00Z</begin>\n' +
      '        <end>2026-07-01T08:05:00Z</end>\n' +
      '      </TimeSpan>\n' +
      '      <LineString>\n' +
      '        <tessellate>1</tessellate>\n' +
      '        <altitudeMode>clampToGround</altitudeMode>\n' +
      '        <coordinates>5.1,52.1,10 5.102,52.101,12</coordinates>\n' +
      '      </LineString>\n' +
      '    </Placemark>\n' +
      '  </Document>\n' +
      '</kml>',
  );
});

test('gpx-to-kml page: controls update colors, width, opacity, altitude, and document name', async ({ page }) => {
  await page.goto('/tools/gpx-to-kml/');
  await page.fill('#in-gpx', WAYPOINT);
  await page.fill('#in-line_color', '#f00');
  await page.fill('#in-line_width', '2');
  await page.fill('#in-line_opacity', '100');
  await page.fill('#in-waypoint_color', '#00f');
  await page.selectOption('#in-altitude_mode', 'absolute');
  await page.fill('#in-document_name', 'Stops');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<name>Stops</name>', { timeout: 15000 });
  await expect(out).toContainText('<color>ff0000ff</color>');
  await expect(out).toContainText('<width>2</width>');
  await expect(out).toContainText('<color>ffff0000</color>');
  await expect(out).toContainText('<altitudeMode>absolute</altitudeMode>');
  await expect(out).toContainText('<coordinates>2,1</coordinates>');
});

test('gpx-to-kml page: query-param deep-link prefills and computes', async ({ page }) => {
  await page.goto(
    '/tools/gpx-to-kml/?gpx=' +
      encodeURIComponent(WAYPOINT) +
      '&line_color=%23f00&line_width=2&line_opacity=100&waypoint_color=%2300f&altitude_mode=absolute&document_name=Stops',
  );
  await expect(page.locator('#in-gpx')).toHaveValue(WAYPOINT, { timeout: 15000 });
  await expect(page.locator('#in-altitude_mode')).toHaveValue('absolute');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<name>Stops</name>', { timeout: 15000 });
  await expect(out).toContainText('<color>ffff0000</color>');
});

import { test, expect } from './fixtures';

// A trailing newline from the CSV writer is trimmed before comparing so the
// assertions pin the exact row/column structure a user would see.
async function outputText(page): Promise<string> {
  const t = await page.locator('#tool-output').textContent();
  return (t ?? '').replace(/\s+$/, '');
}

const TRACK = [
  '<gpx><trk><trkseg>',
  '<trkpt lat="40.000000" lon="-105.000000"><ele>1600</ele><time>2024-03-09T08:00:00Z</time></trkpt>',
  '<trkpt lat="40.001000" lon="-105.000000"><ele>1610</ele><time>2024-03-09T08:01:00Z</time></trkpt>',
  '</trkseg></trk></gpx>',
].join('');

const WAYPOINTS =
  '<gpx><wpt lat="48.8584" lon="2.2945"><name>Eiffel Tower</name></wpt>' +
  '<wpt lat="48.8606" lon="2.3376"><name>Louvre</name></wpt></gpx>';

test('gpx-to-csv page: track → CSV with default columns (ISO time, header on)', async ({
  page,
}) => {
  await page.goto('/tools/gpx-to-csv/');
  await page.fill('#in-gpx', TRACK);
  await expect(page.locator('#tool-output')).toContainText('40.000000', {
    timeout: 15000,
  });
  expect(await outputText(page)).toBe(
    'point_type,name,latitude,longitude,elevation_m,time\n' +
      'track,,40.000000,-105.000000,1600,2024-03-09T08:00:00Z\n' +
      'track,,40.001000,-105.000000,1610,2024-03-09T08:01:00Z',
  );
});

test('gpx-to-csv page: waypoint filter (select) + time_format=none', async ({
  page,
}) => {
  await page.goto('/tools/gpx-to-csv/');
  await page.fill('#in-gpx', WAYPOINTS);
  await page.selectOption('#in-points', 'waypoint');
  await page.selectOption('#in-time_format', 'none');
  await expect(page.locator('#tool-output')).toContainText('Eiffel Tower', {
    timeout: 15000,
  });
  expect(await outputText(page)).toBe(
    'point_type,name,latitude,longitude,elevation_m\n' +
      'waypoint,Eiffel Tower,48.8584,2.2945,\n' +
      'waypoint,Louvre,48.8606,2.3376,',
  );
});

test('gpx-to-csv page: semicolon delimiter (select) reflows separators', async ({
  page,
}) => {
  await page.goto('/tools/gpx-to-csv/');
  await page.fill('#in-gpx', TRACK);
  await page.selectOption('#in-delimiter', 'semicolon');
  await expect(page.locator('#tool-output')).toContainText(
    'point_type;name;latitude',
    { timeout: 15000 },
  );
});

test('gpx-to-csv page: unix time_format (select) emits epoch seconds', async ({
  page,
}) => {
  await page.goto('/tools/gpx-to-csv/');
  await page.fill('#in-gpx', TRACK);
  await page.selectOption('#in-time_format', 'unix');
  // 2024-03-09T08:00:00Z == 1709971200
  await expect(page.locator('#tool-output')).toContainText(',1709971200', {
    timeout: 15000,
  });
});

test('gpx-to-csv page: header checkbox OFF drops the header row (non-default)', async ({
  page,
}) => {
  await page.goto('/tools/gpx-to-csv/');
  await page.fill('#in-gpx', TRACK);
  // header defaults ON — uncheck to exercise the off-path.
  await page.uncheck('#in-header');
  await expect(page.locator('#tool-output')).toContainText('track,,40.000000', {
    timeout: 15000,
  });
  const out = await outputText(page);
  expect(out.startsWith('point_type')).toBe(false);
  expect(out.split('\n').length).toBe(2);
});

test('gpx-to-csv page: derived-speed checkbox ON adds speed_kmh (non-default)', async ({
  page,
}) => {
  await page.goto('/tools/gpx-to-csv/');
  await page.fill('#in-gpx', TRACK);
  await page.check('#in-speed');
  await expect(page.locator('#tool-output')).toContainText(',speed_kmh', {
    timeout: 15000,
  });
  const out = await outputText(page);
  // header ends with the speed column; second data row carries a numeric speed.
  expect(out.split('\n')[0].endsWith(',speed_kmh')).toBe(true);
  expect(out).toMatch(/,6\.\d/);
});

test('gpx-to-csv page: query-param deep-link prefills + computes', async ({
  page,
}) => {
  await page.goto(
    '/tools/gpx-to-csv/?gpx=' +
      encodeURIComponent(WAYPOINTS) +
      '&points=waypoint&time_format=none',
  );
  await expect(page.locator('#in-gpx')).toHaveValue(WAYPOINTS, {
    timeout: 15000,
  });
  await expect(page.locator('#in-points')).toHaveValue('waypoint');
  await expect(page.locator('#tool-output')).toContainText(
    'waypoint,Eiffel Tower,48.8584,2.2945',
    { timeout: 15000 },
  );
});

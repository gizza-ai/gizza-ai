import { test, expect } from './fixtures';

const TCX =
  '<?xml version="1.0"?>\n' +
  '<TrainingCenterDatabase><Activities><Activity Sport="Running">\n' +
  '<Id>2026-07-01T08:00:00Z</Id>\n' +
  '<Lap><Track>\n' +
  '<Trackpoint><Time>2026-07-01T08:00:00Z</Time><Position><LatitudeDegrees>52.100</LatitudeDegrees><LongitudeDegrees>5.100</LongitudeDegrees></Position><AltitudeMeters>10</AltitudeMeters><HeartRateBpm><Value>120</Value></HeartRateBpm><Cadence>82</Cadence></Trackpoint>\n' +
  '<Trackpoint><Time>2026-07-01T08:05:00Z</Time><Position><LatitudeDegrees>52.101</LatitudeDegrees><LongitudeDegrees>5.102</LongitudeDegrees></Position><AltitudeMeters>12</AltitudeMeters><HeartRateBpm><Value>130</Value></HeartRateBpm></Trackpoint>\n' +
  '</Track></Lap></Activity></Activities></TrainingCenterDatabase>';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

test('tcx-to-gpx converts a TCX activity to GPX with exact trackpoint output', async ({ page }) => {
  await page.goto('/tools/tcx-to-gpx/');
  await page.fill('#in-input', TCX);
  await expect(page.locator('#tool-output')).toContainText('<gpx version="1.1"', { timeout: 15000 });
  const out = await output(page);
  expect(out).toContain('<name>2026-07-01T08:00:00Z</name>');
  expect(out).toContain('<type>Running</type>');
  expect(out).toContain('<trkpt lat="52.1" lon="5.1">');
  expect(out).toContain('<ele>10</ele>');
  expect(out).toContain('<time>2026-07-01T08:05:00Z</time>');
  expect(out).toContain('<gpxtpx:hr>120</gpxtpx:hr>');
  expect(out).toContain('<gpxtpx:cad>82</gpxtpx:cad>');
});

test('tcx-to-gpx can omit GPX TrackPointExtension data', async ({ page }) => {
  await page.goto('/tools/tcx-to-gpx/');
  await page.fill('#in-input', TCX);
  await page.uncheck('#in-include_extensions');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<trkpt lat="52.1" lon="5.1">', { timeout: 15000 });
  await expect(out).not.toContainText('gpxtpx');
});

test('tcx-to-gpx pre-fills and auto-runs from query params', async ({ page }) => {
  await page.goto('/tools/tcx-to-gpx/?input=' + encodeURIComponent(TCX) + '&include_extensions=false');
  await expect(page.locator('#in-input')).toHaveValue(TCX, { timeout: 15000 });
  await expect(page.locator('#in-include_extensions')).not.toBeChecked();
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<gpx version="1.1"', { timeout: 15000 });
  await expect(out).not.toContainText('gpxtpx');
});

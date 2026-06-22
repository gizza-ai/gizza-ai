import { test, expect } from './fixtures';

const GPX =
  '<?xml version="1.0"?>\n' +
  '<gpx version="1.1"><trk><name>Test Run</name><trkseg>\n' +
  '<trkpt lat="40.0" lon="-105.0"><ele>1600</ele><time>2024-03-09T08:00:00Z</time></trkpt>\n' +
  '<trkpt lat="40.001" lon="-105.0"><ele>1610</ele><time>2024-03-09T08:01:00Z</time></trkpt>\n' +
  '<trkpt lat="40.002" lon="-105.0"><ele>1605</ele><time>2024-03-09T08:02:00Z</time></trkpt>\n' +
  '</trkseg></trk></gpx>';

// /tools/gpx-analyzer/ analyzes a GPX track in-browser (pure wasm).
test('gpx-analyzer reports distance, elevation, duration and pace', async ({ page }) => {
  await page.goto('/tools/gpx-analyzer/');
  await page.fill('#in-gpx', GPX);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Track: Test Run', { timeout: 15000 });
  await expect(out).toContainText('Points:    3');
  await expect(out).toContainText('Distance:  0.22 km');
  await expect(out).toContainText('+10 m / -5 m');
  await expect(out).toContainText('Duration:  0:02:00');
  await expect(out).toContainText('Max speed:');
  await expect(out).toContainText('min/km');
});

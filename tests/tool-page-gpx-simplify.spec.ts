import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const GPX =
  '<gpx><trk><trkseg>\n' +
  '<trkpt lat="0" lon="0"><ele>1</ele><time>2026-08-06T00:00:00Z</time></trkpt>\n' +
  '<trkpt lat="0" lon="0.0001"></trkpt>\n' +
  '<trkpt lat="0.001" lon="0.001"></trkpt>\n' +
  '<trkpt lat="0" lon="0.002"></trkpt>\n' +
  '</trkseg></trk></gpx>';

test('simplifies GPX output while preserving endpoints and kept metadata', async ({ page }) => {
  await page.goto('/tools/gpx-simplify/');
  await page.fill('#in-input', GPX);
  await page.fill('#in-tolerance_meters', '200');
  await page.selectOption('#in-output', 'gpx');
  await expect(page.locator('#tool-output')).toContainText('<trkpt lat="0" lon="0">', { timeout: 15000 });
  const text = await output(page);
  expect(text).toContain('<ele>1</ele>');
  expect(text).toContain('<time>2026-08-06T00:00:00Z</time>');
  expect(text).toContain('<trkpt lat="0" lon="0.002">');
  expect(text).not.toContain('0.0001');
});

test('supports summary output, coordinate precision, keep-every, and deep links', async ({ page }) => {
  await page.goto(
    '/tools/gpx-simplify/?input=' +
      encodeURIComponent(GPX) +
      '&tolerance_meters=10000&keep_every=2&decimals=3&output=summary',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('GPX simplify: 4 → 3 points', { timeout: 15000 });
  await expect(out).toContainText('Tolerance: 10000 m');
});

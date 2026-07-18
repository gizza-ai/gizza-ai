import { test, expect } from './fixtures';

async function output(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trimEnd();
}

const TWO_GPX =
  '<gpx><trk><name>Morning</name><trkseg>' +
  '<trkpt lat="1" lon="2"><time>2024-01-01T08:00:00Z</time></trkpt>' +
  '</trkseg></trk></gpx>\n' +
  '<gpx><trk><name>Evening</name><trkseg>' +
  '<trkpt lat="3" lon="4"><time>2024-01-01T07:00:00Z</time></trkpt>' +
  '</trkseg></trk></gpx>';

test('merges two GPX files into one chronological track (exact output)', async ({ page }) => {
  await page.goto('/tools/gpx-merge/');
  await page.fill('#in-input', TWO_GPX);
  await expect(page.locator('#tool-output')).toContainText('<trkpt lat="3" lon="4">', { timeout: 15000 });
  expect(await output(page)).toBe(
    '<?xml version="1.0" encoding="UTF-8"?>\n' +
      '<gpx version="1.1" creator="gizza-ai/gpx-merge" xmlns="http://www.topografix.com/GPX/1/1">\n' +
      '  <trk>\n' +
      '    <name>Merged track</name>\n' +
      '    <trkseg>\n' +
      '      <trkpt lat="3" lon="4">\n' +
      '        <time>2024-01-01T07:00:00Z</time>\n' +
      '      </trkpt>\n' +
      '      <trkpt lat="1" lon="2">\n' +
      '        <time>2024-01-01T08:00:00Z</time>\n' +
      '      </trkpt>\n' +
      '    </trkseg>\n' +
      '  </trk>\n' +
      '</gpx>',
  );
});

test('keeps source tracks when merge_mode is separate-tracks (enum surface)', async ({ page }) => {
  await page.goto('/tools/gpx-merge/');
  await page.selectOption('#in-merge_mode', 'separate-tracks');
  await page.fill('#in-input', TWO_GPX);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<name>Evening</name>', { timeout: 15000 });
  await expect(out).toContainText('<name>Morning</name>');
  expect((await output(page)).match(/<trk>/g)?.length).toBe(2);
});

test('honors non-default checkbox states for sort, dedupe, and waypoints', async ({ page }) => {
  await page.goto('/tools/gpx-merge/');
  await expect(page.locator('#in-sort_by_time')).toBeChecked();
  await expect(page.locator('#in-include_waypoints')).toBeChecked();
  await page.uncheck('#in-sort_by_time');
  await page.check('#in-dedupe');
  await page.uncheck('#in-include_waypoints');
  await page.fill(
    '#in-input',
    '<gpx><wpt lat="9" lon="9"><name>Camp</name></wpt><trk><trkseg>' +
      '<trkpt lat="1" lon="2"><time>2024-01-01T08:00:00Z</time></trkpt>' +
      '<trkpt lat="1" lon="2"><time>2024-01-01T08:00:00Z</time></trkpt>' +
      '<trkpt lat="3" lon="4"><time>2024-01-01T07:00:00Z</time></trkpt>' +
      '</trkseg></trk></gpx>',
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<trkpt lat="1" lon="2">', { timeout: 15000 });
  const text = await output(page);
  expect(text).not.toContain('<wpt');
  expect(text.match(/<trkpt/g)?.length).toBe(2);
  expect(text.indexOf('lat="1"')).toBeLessThan(text.indexOf('lat="3"'));
});

test('pre-fills and auto-runs from query params (deep link)', async ({ page }) => {
  const gpx = '<gpx><rte><name>Route A</name><rtept lat="1" lon="1"/><rtept lat="2" lon="2"/></rte></gpx>';
  await page.goto('/tools/gpx-merge/?input=' + encodeURIComponent(gpx) + '&merge_mode=single-track-multi-segment');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<trkpt lat="1" lon="1">', { timeout: 15000 });
  await expect(out).toContainText('<trkseg>');
});

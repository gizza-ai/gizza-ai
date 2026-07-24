import { test, expect } from './fixtures';

const TRACK = '<gpx><trk><name>Demo</name><trkseg>' +
  '<trkpt lat="0.000" lon="0"><time>2024-01-01T00:00:00Z</time></trkpt>' +
  '<trkpt lat="0.010" lon="0"><time>2024-01-01T00:10:00Z</time></trkpt>' +
  '<trkpt lat="0.020" lon="0"><time>2024-01-01T00:20:00Z</time></trkpt>' +
  '</trkseg></trk></gpx>';

const STOP_TRACK = '<gpx><trk><name>Stop Demo</name><trkseg>' +
  '<trkpt lat="0.000" lon="0"><time>2024-01-01T00:00:00Z</time></trkpt>' +
  '<trkpt lat="0.001" lon="0"><time>2024-01-01T00:01:00Z</time></trkpt>' +
  '<trkpt lat="0.002" lon="0"><time>2024-01-01T00:21:00Z</time></trkpt>' +
  '<trkpt lat="0.003" lon="0"><time>2024-01-01T00:22:00Z</time></trkpt>' +
  '</trkseg></trk></gpx>';

async function setGpx(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-gpx').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('gpx-split distance summary has exact segment count and units', async ({ page }) => {
  await page.goto('/tools/gpx-split/');
  await setGpx(page, TRACK);
  await page.fill('#in-distance', '1');
  await page.selectOption('#in-output', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Split into 2 segments (distance, every 1 km).', { timeout: 15_000 });
  await expect(out).toContainText('Segment 1: 1.11 km (0.69 mi), 2 points, 0:10:00');
  await expect(out).toContainText('Segment 2: 1.11 km (0.69 mi), 2 points, 0:10:00');
});

test('gpx-split deep-links time mode', async ({ page }) => {
  const qs = new URLSearchParams({
    gpx: TRACK,
    mode: 'time',
    time_min: '10',
    output: 'summary',
  });
  await page.goto(`/tools/gpx-split/?${qs.toString()}`);

  await expect(page.locator('#in-mode')).toHaveValue('time');
  await expect(page.locator('#in-time_min')).toHaveValue('10');
  await expect(page.locator('#in-output')).toHaveValue('summary');
  await expect(page.locator('#tool-output')).toContainText('Split into 2 segments (time, every 10 min).', { timeout: 15_000 });
});

test('gpx-split stops mode detects a pause gap', async ({ page }) => {
  await page.goto('/tools/gpx-split/');
  await setGpx(page, STOP_TRACK);
  await page.selectOption('#in-mode', 'stops');
  await page.fill('#in-stop_gap_s', '120');
  await page.selectOption('#in-output', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Split into 2 segments (stops, on gaps over 120 s).', { timeout: 15_000 });
  await expect(out).toContainText('Segment 1: 0.11 km (0.07 mi), 2 points, 0:01:00');
  await expect(out).toContainText('Segment 2: 0.11 km (0.07 mi), 2 points, 0:01:00');
});

test('gpx-split covers unit miles and GPX output', async ({ page }) => {
  await page.goto('/tools/gpx-split/');
  await setGpx(page, TRACK);
  await page.fill('#in-distance', '0.5');
  await page.selectOption('#in-unit', 'mi');
  await page.selectOption('#in-output', 'gpx');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<?xml version="1.0" encoding="UTF-8"?>', { timeout: 15_000 });
  await expect(out).toContainText('<name>Demo - Part 1</name>');
  await expect(out).toContainText('<name>Demo - Part 2</name>');
});

test('gpx-split reports missing timestamps for time mode', async ({ page }) => {
  await page.goto('/tools/gpx-split/');
  await setGpx(page, '<gpx><trk><trkseg><trkpt lat="0" lon="0"/><trkpt lat="0.01" lon="0"/></trkseg></trk></gpx>');
  await page.selectOption('#in-mode', 'time');
  await page.selectOption('#in-output', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('time mode needs <time> stamps', { timeout: 15_000 });
  await expect(out).toHaveClass(/error/);
});

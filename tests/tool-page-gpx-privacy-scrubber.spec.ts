import { test, expect } from './fixtures';

async function outputText(page): Promise<string> {
  return ((await page.locator('#tool-output').textContent()) ?? '').trim();
}

const SAMPLE_GPX =
  '<gpx version="1.1"><trk><trkseg><trkpt lat="52.100000" lon="4.100000"><time>2024-01-01T08:00:00Z</time></trkpt><trkpt lat="52.200000" lon="4.200000"><time>2024-01-01T08:05:00Z</time><extensions><hr>150</hr></extensions></trkpt><trkpt lat="52.300000" lon="4.300000"><time>2024-01-01T08:10:00Z</time></trkpt></trkseg></trk></gpx>';

const DEFAULT_OUTPUT =
  '<gpx version="1.1"><trk><trkseg><trkpt lat="52.200000" lon="4.200000"><time>1970-01-01T00:00:00Z</time></trkpt></trkseg></trk></gpx>';

test('gpx-privacy-scrubber page — default removes endpoints, times, and extensions', async ({
  page,
}) => {
  await page.goto('/tools/gpx-privacy-scrubber/');
  await page.fill('#in-gpx', SAMPLE_GPX);
  await expect(page.locator('#tool-output')).toContainText('52.200000', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_OUTPUT);
});

test('gpx-privacy-scrubber page — fuzz mode keeps point count and moves endpoints', async ({
  page,
}) => {
  await page.goto('/tools/gpx-privacy-scrubber/');
  await page.fill('#in-gpx', SAMPLE_GPX);
  await page.selectOption('#in-mode', 'fuzz');
  await page.fill('#in-radius_m', '500');
  await page.uncheck('#in-scrub_timestamps');
  await page.uncheck('#in-remove_extensions');
  await expect(page.locator('#tool-output')).toContainText('<extensions><hr>150</hr></extensions>', {
    timeout: 15000,
  });
  const out = await outputText(page);
  expect((out.match(/<trkpt/g) ?? []).length).toBe(3);
  expect(out).toContain('lat="52.200000" lon="4.200000"');
  expect(out).not.toContain('lat="52.100000" lon="4.100000"');
  expect(out).not.toContain('lat="52.300000" lon="4.300000"');
  expect(out).toContain('2024-01-01T08:05:00Z');
});

test('gpx-privacy-scrubber page — query-param deep-link prefills and auto-runs', async ({
  page,
}) => {
  const url =
    '/tools/gpx-privacy-scrubber/?gpx=' +
    encodeURIComponent(SAMPLE_GPX) +
    '&mode=remove&radius_m=10000&scrub_timestamps=true&remove_extensions=true';
  await page.goto(url);
  await expect(page.locator('#in-gpx')).toHaveValue(SAMPLE_GPX, { timeout: 15000 });
  await expect(page.locator('#tool-output')).toContainText('52.200000', { timeout: 15000 });
  expect(await outputText(page)).toBe(DEFAULT_OUTPUT);
});

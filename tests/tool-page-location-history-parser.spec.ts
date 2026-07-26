import { test, expect } from './fixtures';

const semantic = '{"timelineObjects":[{"placeVisit":{"location":{"latitudeE7":400000000,"longitudeE7":-740000000,"name":"Home"},"duration":{"startTimestamp":"2024-01-01T08:00:00Z"}}},{"activitySegment":{"distance":5000,"duration":{"startTimestamp":"2024-01-01T09:00:00Z"}}},{"placeVisit":{"location":{"latitudeE7":401000000,"longitudeE7":-740000000,"name":"Office"},"duration":{"startTimestamp":"2024-01-01T10:00:00Z"}}}]}';

// /tools/location-history-parser/ summarizes Google Takeout / Dawarich JSON in-browser.
test('location-history-parser renders exact text summary for semantic history', async ({ page }) => {
  await page.goto('/tools/location-history-parser/');
  await page.fill('#in-input', semantic);
  await page.selectOption('#in-output', 'text');
  await page.selectOption('#in-unit', 'km');
  await page.fill('#in-utc_offset', '0');
  await page.fill('#in-place_radius', '150');
  await page.fill('#in-min_stay', '10');

  await expect(page.locator('#tool-output')).toHaveText(
    '2024-01-01 — 2 places, 5.00 km\n\nTotal over 1 day: 2 places, 5.00 km',
    { timeout: 15000 },
  );
});

test('location-history-parser query-param deep-link renders miles CSV', async ({ page }) => {
  await page.goto(
    '/tools/location-history-parser/?input=' +
      encodeURIComponent(semantic) +
      '&output=csv&unit=mi&utc_offset=0&place_radius=150&min_stay=10',
  );
  await expect(page.locator('#in-input')).toHaveValue(semantic, { timeout: 15000 });
  await expect(page.locator('#in-output')).toHaveValue('csv');
  await expect(page.locator('#in-unit')).toHaveValue('mi');
  await expect(page.locator('#tool-output')).toHaveText(
    'date,places,distance_mi\n2024-01-01,2,3.11',
    { timeout: 15000 },
  );
});

test('location-history-parser reports invalid JSON', async ({ page }) => {
  await page.goto('/tools/location-history-parser/');
  await page.fill('#in-input', '{bad}');
  await expect(page.locator('#tool-output')).toContainText('invalid JSON', { timeout: 15000 });
});

test('location-history-parser raw point array can emit JSON', async ({ page }) => {
  const points = '[{"latitude":40.0,"longitude":-74.0,"timestamp":1704067200},{"latitude":40.5,"longitude":-74.0,"timestamp":1704070800}]';
  await page.goto('/tools/location-history-parser/');
  await page.fill('#in-input', points);
  await page.selectOption('#in-output', 'json');
  await page.selectOption('#in-unit', 'km');
  await page.fill('#in-place_radius', '1');
  await page.fill('#in-min_stay', '0');
  await expect(page.locator('#tool-output')).toContainText(
    '"source_format": "Location point array (Dawarich / generic)"',
    { timeout: 15000 },
  );
  await expect(page.locator('#tool-output')).toContainText('"date": "2024-01-01"');
});

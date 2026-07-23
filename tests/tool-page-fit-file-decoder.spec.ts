import { test, expect } from './fixtures';

// A real, self-consistent FIT activity (3 GPS records + a cycling session),
// base64-encoded. Same fixture the core Rust tests use.
const SAMPLE =
  'DhBcCMAAAAAuRklUz5dAAAAUAAn9BIYABIUBBIUCAoQFBIYGAoQDAQIEAQIHAoQAAMtOQBzHcRxVVVW1BCkAAAAAiBN4UJYAADzLTkC39XEcVVVVtTYpXCsAAHAXjFXIAAB4y05AUSRyHLsmVbVoKahhAABYG5ZY+gBBAAASAA79BIYFAQACBIYHBIYIBIYJBIYLAoQOAoQPAoQQAQIRAQIUAoQVAoQWAoQBeMtOQAIAy05AwNQBAMDUAQCoYQAAKgBwF1gbiZbIAPoAFAAPJg==';

async function setData(page: import('@playwright/test').Page, value: string) {
  await page.locator('#in-data').evaluate((el, v) => {
    (el as HTMLTextAreaElement).value = v;
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}

test('fit-file-decoder summary shows session totals and averages', async ({ page }) => {
  await page.goto('/tools/fit-file-decoder/');
  await setData(page, SAMPLE);
  await page.selectOption('#in-format', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('Records decoded:  3', { timeout: 15_000 });
  await expect(out).toContainText('Protocol version: 1.0');
  await expect(out).toContainText('Profile version:  21.40');
  await expect(out).toContainText(
    'Time range:       2024-03-09T08:00:00Z → 2024-03-09T08:02:00Z (2:00)',
  );
  await expect(out).toContainText('Sport:            cycling');
  await expect(out).toContainText('Total distance:   0.25 km');
  await expect(out).toContainText('Speed:            avg 21.6  max 25.2 (km/h)');
  await expect(out).toContainText('Heart rate:       avg 137  max 150 bpm');
  await expect(out).toContainText('Power:            avg 200  max 250 W');
});

test('fit-file-decoder CSV has the exact header and first row', async ({ page }) => {
  await page.goto('/tools/fit-file-decoder/');
  await setData(page, SAMPLE);
  await page.selectOption('#in-format', 'csv');

  const out = page.locator('#tool-output');
  await expect(out).toContainText(
    'timestamp,latitude,longitude,altitude_m,distance_m,speed_mps,heart_rate,cadence,power',
    { timeout: 15_000 },
  );
  await expect(out).toContainText(
    '2024-03-09T08:00:00Z,40.0000000,-105.0000000,1600.0,0.00,5.000,120,80,150',
  );
  await expect(out).toContainText(
    '2024-03-09T08:02:00Z,40.0020000,-105.0010000,1620.0,250.00,7.000,150,88,250',
  );
});

test('fit-file-decoder GPX has three trackpoints with sensor extensions', async ({ page }) => {
  await page.goto('/tools/fit-file-decoder/');
  await setData(page, SAMPLE);
  await page.selectOption('#in-format', 'gpx');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('<gpx version="1.1"', { timeout: 15_000 });
  await expect(out).toContainText('lat="40.0000000" lon="-105.0000000"');
  await expect(out).toContainText('<gpxtpx:hr>120</gpxtpx:hr>');
  await expect(out).toContainText('<gpxtpx:cad>80</gpxtpx:cad>');
  await expect(out).toContainText('<power>150</power>');

  const trkpts = await out.evaluate(
    (el) => (el.textContent || '').split('<trkpt').length - 1,
  );
  expect(trkpts).toBe(3);
});

test('fit-file-decoder deep-links the CSV format', async ({ page }) => {
  const qs = new URLSearchParams({ data: SAMPLE, format: 'csv' });
  await page.goto(`/tools/fit-file-decoder/?${qs.toString()}`);

  await expect(page.locator('#in-format')).toHaveValue('csv');
  await expect(page.locator('#tool-output')).toContainText(
    'timestamp,latitude,longitude,altitude_m',
    { timeout: 15_000 },
  );
  await expect(page.locator('#tool-output')).toContainText(
    '2024-03-09T08:01:00Z,40.0010000,-105.0000000,1610.0,111.00,6.000,140,85,200',
  );
});

test('fit-file-decoder reports invalid base64 input', async ({ page }) => {
  await page.goto('/tools/fit-file-decoder/');
  await setData(page, '@@@@');
  await page.selectOption('#in-format', 'summary');

  const out = page.locator('#tool-output');
  await expect(out).toContainText('invalid base64', { timeout: 15_000 });
  await expect(out).toHaveClass(/error/);
});

import { test, expect } from './fixtures';

const JSON_INPUT = '[{"_type":"location","tid":"5f","acc":12,"batt":56,"vel":9,"cog":180,"lat":52.1,"lon":5.1,"alt":10,"tst":1704110400},{"_type":"location","tid":"5f","acc":8,"lat":52.101,"lon":5.102,"alt":12,"tst":1704110700},{"_type":"transition","lat":0,"lon":0,"tst":1704110800}]';

const REC_INPUT = '2024-01-01T12:00:00Z\t*\t{"_type":"location","lat":1.0,"lon":2.0}\n2024-01-01T12:05:00Z\t*\t{"_type":"location","lat":1.1,"lon":2.1}\n2024-01-01T13:00:00Z\t*\t{"_type":"location","lat":1.2,"lon":2.2}';

test('owntracks-to-gpx page converts JSON export to GPX with extensions', async ({ page }) => {
  await page.goto('/tools/owntracks-to-gpx/');
  await page.fill('#in-track_name', 'Morning walk');
  await page.fill('#in-input', JSON_INPUT);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<?xml version="1.0" encoding="UTF-8"?>', { timeout: 15000 });
  await expect(out).toContainText('<name>Morning walk</name>');
  await expect(out).toContainText('<trkpt lat="52.1" lon="5.1">');
  await expect(out).toContainText('<time>2024-01-01T12:00:00Z</time>');
  await expect(out).toContainText('<ot:accuracy>12</ot:accuracy>');
  expect(((await out.textContent())!.match(/<trkpt /g) || []).length).toBe(2);
});

test('owntracks-to-gpx page parses .rec lines and splits segments', async ({ page }) => {
  await page.goto('/tools/owntracks-to-gpx/');
  await page.uncheck('#in-include_extensions');
  await page.fill('#in-segment_gap_minutes', '20');
  await page.fill('#in-input', REC_INPUT);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<trkpt lat="1" lon="2">', { timeout: 15000 });
  await expect(out).toContainText('<time>2024-01-01T12:00:00Z</time>');
  expect(((await out.textContent())!.match(/<trkseg>/g) || []).length).toBe(2);
  await expect(out).not.toContainText('xmlns:ot=');
});

test('owntracks-to-gpx deep-link pre-fills options and accuracy filter drops a bad fix', async ({ page }) => {
  await page.goto('/tools/owntracks-to-gpx/?track_name=Filtered&include_extensions=false&max_accuracy_meters=50&segment_gap_minutes=0');
  await expect(page.locator('#in-track_name')).toHaveValue('Filtered');
  await expect(page.locator('#in-include_extensions')).not.toBeChecked();
  await expect(page.locator('#in-max_accuracy_meters')).toHaveValue('50');
  await page.fill(
    '#in-input',
    '[{"_type":"location","lat":48.2,"lon":16.37,"acc":8,"tst":1704110400},{"_type":"location","lat":48.21,"lon":16.38,"acc":250,"tst":1704110700},{"_type":"location","lat":48.22,"lon":16.39,"tst":1704111000}]'
  );
  const out = page.locator('#tool-output');
  await expect(out).toContainText('<name>Filtered</name>', { timeout: 15000 });
  expect(((await out.textContent())!.match(/<trkpt /g) || []).length).toBe(2);
  await expect(out).not.toContainText('16.38');
});

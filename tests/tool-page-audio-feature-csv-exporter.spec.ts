import { test, expect } from './fixtures';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.wav');

function parseCsv(text: string): string[][] {
  return text
    .trim()
    .split(/\r?\n/)
    .map((line) => line.split(','));
}

test('audio-feature-csv-exporter page exports real CSV from uploaded audio', async ({ page }) => {
  await page.goto('/tools/audio-feature-csv-exporter/');
  await page.waitForSelector('#in-input');
  await page.setInputFiles('#in-input', FIXTURE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('time_s,rms_dbfs,centroid_hz,zcr,rolloff_hz,flatness', { timeout: 45_000 });

  const csv = await out.innerText();
  const rows = parseCsv(csv);
  expect(rows.length).toBeGreaterThan(250);
  expect(rows[0]).toEqual(['time_s', 'rms_dbfs', 'centroid_hz', 'zcr', 'rolloff_hz', 'flatness']);
  expect(Number(rows[1][0])).toBeCloseTo(0, 6);
  expect(Number(rows[1][1])).toBeLessThan(0);
  expect(Number(rows[30][2])).toBeGreaterThan(300);
  expect(Number(rows[30][2])).toBeLessThan(600);
  expect(Number(rows[30][5])).toBeGreaterThanOrEqual(0);
  expect(Number(rows[30][5])).toBeLessThanOrEqual(1);
});

test('audio-feature-csv-exporter deep link applies non-default feature controls', async ({ page }) => {
  await page.goto('/tools/audio-feature-csv-exporter/?output=csv&frame_ms=40&hop_ms=20&window=hamming&center=true&channel=left&resample_hz=16000&rms=true&centroid=false&zcr=false&rolloff=false&flatness=false&bandwidth=true&flux=true&rolloff_percent=95&rms_scale=linear&flatness_scale=db&include_time=false&include_frame=true&decimals=4');
  await page.waitForSelector('#in-input');

  await expect(page.locator('#in-frame_ms')).toHaveValue('40');
  await expect(page.locator('#in-hop_ms')).toHaveValue('20');
  await expect(page.locator('#in-window')).toHaveValue('hamming');
  await expect(page.locator('#in-center')).toBeChecked();
  await expect(page.locator('#in-channel')).toHaveValue('left');
  await expect(page.locator('#in-resample_hz')).toHaveValue('16000');
  await expect(page.locator('#in-centroid')).not.toBeChecked();
  await expect(page.locator('#in-bandwidth')).toBeChecked();
  await expect(page.locator('#in-flux')).toBeChecked();
  await expect(page.locator('#in-include_time')).not.toBeChecked();
  await expect(page.locator('#in-include_frame')).toBeChecked();

  await page.setInputFiles('#in-input', FIXTURE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('frame,rms,bandwidth_hz,flux', { timeout: 45_000 });
  const rows = parseCsv(await out.innerText());
  expect(rows[0]).toEqual(['frame', 'rms', 'bandwidth_hz', 'flux']);
  expect(rows.length).toBeGreaterThan(100);
  expect(Number(rows[1][0])).toBe(0);
  expect(Number(rows[10][1])).toBeGreaterThan(0);
  expect(Number(rows[10][2])).toBeGreaterThanOrEqual(0);
});

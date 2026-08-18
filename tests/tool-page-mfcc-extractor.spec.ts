import { test, expect } from './fixtures';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.wav');

test('mfcc-extractor page extracts default CSV coefficients from an uploaded WAV', async ({ page }) => {
  await page.goto('/tools/mfcc-extractor/');
  await page.waitForSelector('#in-input');
  await page.setInputFiles('#in-input', FIXTURE);

  const out = page.locator('#tool-output');
  await expect(out).toContainText('time_s,c0,c1,c2,c3,c4,c5,c6,c7,c8,c9,c10,c11,c12', { timeout: 30_000 });
  await expect(out).toContainText('0.000000,');
  await expect(out).toContainText('0.010000,');
  const text = (await out.textContent()) ?? '';
  const rows = text.trim().split('\n');
  expect(rows.length).toBeGreaterThan(100);
  expect(rows[1].split(',')).toHaveLength(14);
});

test('mfcc-extractor deep link applies JSON, deltas, no energy, no time, and resampling', async ({ page }) => {
  await page.goto('/tools/mfcc-extractor/?output=json&n_mfcc=3&n_mels=8&frame_ms=20&hop_ms=20&window=hann&preemphasis=0&lifter=0&mel_scale=slaney&append_energy=false&deltas=delta_delta&include_time=false&decimals=3&resample_hz=16000');
  await page.waitForSelector('#in-input');

  await expect(page.locator('#in-output')).toHaveValue('json', { timeout: 15_000 });
  await expect(page.locator('#in-n_mfcc')).toHaveValue('3');
  await expect(page.locator('#in-window')).toHaveValue('hann');
  await expect(page.locator('#in-mel_scale')).toHaveValue('slaney');
  await expect(page.locator('#in-append_energy')).not.toBeChecked();
  await expect(page.locator('#in-include_time')).not.toBeChecked();
  await expect(page.locator('#in-deltas')).toHaveValue('delta_delta');
  await expect(page.locator('#in-resample_hz')).toHaveValue('16000');

  await page.setInputFiles('#in-input', FIXTURE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"columns": ["c0", "c1", "c2", "d0", "d1", "d2", "dd0", "dd1", "dd2"]', { timeout: 30_000 });
  await expect(out).toContainText('"sample_rate": 16000');
  await expect(out).toContainText('"n_mfcc": 3');
  await expect(out).toContainText('"deltas": "delta_delta"');
  const text = (await out.textContent()) ?? '';
  expect(text).toContain('"mfcc": [');
  expect(text).not.toContain('time_s');
  const firstRow = text.match(/\[\s*(-?\d+\.\d{3}),\s*(-?\d+\.\d{3}),\s*(-?\d+\.\d{3}),\s*(-?\d+\.\d{3}),\s*(-?\d+\.\d{3}),\s*(-?\d+\.\d{3}),\s*(-?\d+\.\d{3}),\s*(-?\d+\.\d{3}),\s*(-?\d+\.\d{3})\s*\]/);
  expect(firstRow).not.toBeNull();
});

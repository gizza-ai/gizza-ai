import { test, expect } from './fixtures';

const SAMPLE_B64 =
  'UklGRjQAAABXQVZFZm10IBAAAAABAAEACAAAABAAAAACABAAZGF0YRAAAADNDJkZ/3//f/9/ZibNDAAA';
const SAMPLE_HEX =
  '524946463400000057415645666d742010000000010001000800000010000000020010006461746110000000cd0c9919ff7fff7fff7f6626cd0c0000';

const EXPECTED_REPORT = `Clipping report
Format:        PCM 16-bit integer WAV (8 Hz, 1 ch, 16-bit)
Duration:      0:01.000 (8 frames)
Threshold:     0.9900 of full scale (-0.09 dBFS)
Peak:          1.0000 (-0.00 dBFS)
Clipped:       3 of 8 samples (37.500%)
Clipped frames:3 of 8 (37.500%)
Longest run:   3 frames (375.0 ms)
Regions:       1 run(s) of >= 1 consecutive clipped frame(s)
Worst 1 region(s):
  1. 0:00.250 - 0:00.625  (3 frames, 375.0 ms, peak -0.00 dBFS)`;

test('clipping-detector page reports clipped samples and timestamps exactly', async ({ page }) => {
  await page.goto('/tools/clipping-detector/');
  await page.fill('#in-input', SAMPLE_B64);

  await expect(page.locator('#tool-output')).toHaveText(EXPECTED_REPORT, {
    timeout: 15_000,
  });
});

test('clipping-detector supports hex input and JSON output', async ({ page }) => {
  await page.goto('/tools/clipping-detector/');
  await page.fill('#in-input', SAMPLE_HEX);
  await page.selectOption('#in-input_format', 'hex');
  await page.selectOption('#in-output', 'json');
  await page.fill('#in-min_run', '3');
  await page.fill('#in-top_regions', '3');

  await expect(page.locator('#tool-output')).toContainText('"region_count":1', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText('"start_time":"0:00.250"');
  await expect(page.locator('#tool-output')).toContainText('"end_time":"0:00.625"');
});

test('clipping-detector deep link prefills params and honors exact threshold', async ({ page }) => {
  const params = new URLSearchParams({
    input: SAMPLE_B64,
    input_format: 'base64',
    output: 'report',
    threshold: '1.0',
    min_run: '1',
    gap: '0',
    top_regions: '1',
  });

  await page.goto(`/tools/clipping-detector/?${params.toString()}`);
  await expect(page.locator('#in-threshold')).toHaveValue('1.0', { timeout: 15_000 });
  await expect(page.locator('#in-top_regions')).toHaveValue('1');
  await expect(page.locator('#tool-output')).toContainText('Threshold:     1.0000 of full scale (0.00 dBFS)', {
    timeout: 15_000,
  });
  await expect(page.locator('#tool-output')).toContainText('Result:        No clipping detected.');
});

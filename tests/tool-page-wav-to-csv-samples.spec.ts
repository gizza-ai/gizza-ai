import { test, expect } from './fixtures';

// 16 kHz mono 16-bit WAV whose first three sample frames are the raw integers
// 16384, -8192, 0 (i.e. normalized 0.5, -0.25, 0.0).
const MONO_B64 =
  'UklGRi4AAABXQVZFZm10IBAAAAABAAEAgD4AAAB9AAACABAAZGF0YQYAAAAAQADgAAA=';
// Stereo 16-bit WAV, interleaved L,R = 16384,-16384, 0,32767.
const STEREO_B64 =
  'UklGRjAAAABXQVZFZm10IBAAAAABAAIAgD4AAAD6AAAEABAAZGF0YQgAAAAAQADAAAD/fw==';

test('wav-to-csv-samples exports normalized float samples with a time index', async ({ page }) => {
  await page.goto('/tools/wav-to-csv-samples/');
  await page.fill('#in-input', MONO_B64);

  await expect(page.locator('#tool-output')).toHaveText(
    'time_s,channel_1\n0.000000,0.500000\n0.000063,-0.250000\n0.000125,0.000000',
    { timeout: 15000 },
  );
});

test('wav-to-csv-samples emits raw integers with a sample index', async ({ page }) => {
  await page.goto('/tools/wav-to-csv-samples/');
  await page.fill('#in-input', MONO_B64);
  await page.selectOption('#in-value_scale', 'int');
  await page.selectOption('#in-index_column', 'sample');

  await expect(page.locator('#tool-output')).toHaveText(
    'sample,channel_1\n0,16384\n1,-8192\n2,0',
    { timeout: 15000 },
  );
});

test('wav-to-csv-samples deep-link prefills params and exports raw stereo integers', async ({ page }) => {
  const params = new URLSearchParams({
    input: STEREO_B64,
    input_format: 'base64',
    value_scale: 'int',
    index_column: 'none',
    delimiter: 'tab',
  });

  await page.goto(`/tools/wav-to-csv-samples/?${params.toString()}`);
  // The deep-linked select values are reflected in the controls.
  await expect(page.locator('#in-value_scale')).toHaveValue('int');
  await expect(page.locator('#in-delimiter')).toHaveValue('tab');

  await expect(page.locator('#tool-output')).toHaveText(
    'channel_1\tchannel_2\n16384\t-16384\n0\t32767',
    { timeout: 15000 },
  );
});

test('wav-to-csv-samples can drop the header row', async ({ page }) => {
  await page.goto('/tools/wav-to-csv-samples/');
  await page.fill('#in-input', MONO_B64);
  await page.selectOption('#in-value_scale', 'int');
  await page.selectOption('#in-index_column', 'none');
  await page.uncheck('#in-header');

  await expect(page.locator('#tool-output')).toHaveText('16384\n-8192\n0', {
    timeout: 15000,
  });
});

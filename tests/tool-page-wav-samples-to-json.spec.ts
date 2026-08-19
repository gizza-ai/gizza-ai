import { test, expect } from './fixtures';

const MONO_WAV = 'UklGRioAAABXQVZFZm10IBAAAAABAAEAgD4AAAB9AAACABAAZGF0YQYAAAAAQADgAAA=';
const MONO_WAV_HEX =
  '524946462a00000057415645666d74201000000001000100803e0000007d0000020010006461746106000000004000e00000';
const STEREO_WAV = 'UklGRiwAAABXQVZFZm10IBAAAAABAAIAgD4AAAD6AAAEABAAZGF0YQgAAAAAQADAAAD/fw==';

async function fillInput(page: import('@playwright/test').Page, value = MONO_WAV) {
  await page.fill('#in-input', value);
}

test('wav-samples-to-json renders default metadata plus samples', async ({ page }) => {
  await page.goto('/tools/wav-samples-to-json/');
  await fillInput(page);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"sampleRate": 16000', { timeout: 15000 });
  await expect(out).toContainText('"channels": 1');
  await expect(out).toContainText('"bitDepth": 16');
  await expect(out).toContainText('"valueScale": "float"');
  await expect(out).toContainText('"samples": [0.500000, -0.250000, 0.000000]');
});

test('wav-samples-to-json samples-only int output is exact', async ({ page }) => {
  await page.goto('/tools/wav-samples-to-json/');
  await fillInput(page);
  await page.selectOption('#in-output', 'samples');
  await page.selectOption('#in-value_scale', 'int');
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), { timeout: 15000 })
    .toBe('[16384, -8192, 0]');
});

test('wav-samples-to-json metadata-only deep-link pre-fills and computes', async ({ page }) => {
  await page.goto(
    '/tools/wav-samples-to-json/?input=' +
      encodeURIComponent(MONO_WAV) +
      '&input_format=base64&output=metadata',
  );
  await expect(page.locator('#in-input')).toHaveValue(MONO_WAV, { timeout: 15000 });
  const out = page.locator('#tool-output');
  await expect(out).toContainText('"sampleRate": 16000', { timeout: 15000 });
  await expect(out).toContainText('"totalFrames": 3');
  await expect(out).not.toContainText('"samples"');
});

test('wav-samples-to-json accepts hex input', async ({ page }) => {
  await page.goto('/tools/wav-samples-to-json/');
  await fillInput(page, MONO_WAV_HEX);
  await page.selectOption('#in-input_format', 'hex');
  await page.selectOption('#in-output', 'samples');
  await page.selectOption('#in-value_scale', 'int');
  await page.fill('#in-indent', '0');
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), { timeout: 15000 })
    .toBe('[16384,-8192,0]');
});

test('wav-samples-to-json deinterleaves stereo channels', async ({ page }) => {
  await page.goto('/tools/wav-samples-to-json/');
  await fillInput(page, STEREO_WAV);
  await page.selectOption('#in-output', 'samples');
  await page.selectOption('#in-layout', 'channels');
  await page.selectOption('#in-value_scale', 'int');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('[16384, 0]', { timeout: 15000 });
  await expect(out).toContainText('[-16384, 32767]');
});

test('wav-samples-to-json numeric controls window and decimate frames', async ({ page }) => {
  await page.goto('/tools/wav-samples-to-json/');
  await fillInput(page);
  await page.selectOption('#in-output', 'samples');
  await page.fill('#in-start_frame', '1');
  await page.fill('#in-frame_step', '2');
  await page.fill('#in-max_frames', '1');
  await page.fill('#in-precision', '3');
  await page.fill('#in-indent', '0');
  await expect
    .poll(async () => (await page.locator('#tool-output').textContent())?.trim(), { timeout: 15000 })
    .toBe('[-0.250]');
});

test('wav-samples-to-json reports non-WAV input clearly', async ({ page }) => {
  await page.goto('/tools/wav-samples-to-json/');
  await fillInput(page, 'bm90YXdhdg==');
  await expect(page.locator('#tool-output')).toContainText(
    'not a WAV file: only 7 bytes, too short for a RIFF header',
    { timeout: 15000 },
  );
});

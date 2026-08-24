import { test, expect } from './fixtures';

const wavBase64 = 'UklGRioAAABXQVZFZm10IBAAAAABAAEAgD4AAAB9AAACABAAZGF0YQYAAAAAQADgAAA=';
const wavHex = '524946462a00000057415645666d74201000000001000100803e0000007d0000020010006461746106000000004000e00000';
const expectedNpyBase64 = 'k05VTVBZAQB2AHsnZGVzY3InOiAnPGY0JywgJ2ZvcnRyYW5fb3JkZXInOiBGYWxzZSwgJ3NoYXBlJzogKDMsKSwgfSAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAoAAAA/AACAvgAAAAA=';

test('wav-to-numpy-npy emits exact base64 NPY for the placeholder WAV', async ({ page }) => {
  await page.goto('/tools/wav-to-numpy-npy/');
  await page.fill('#in-input', wavBase64);
  const out = page.locator('#tool-output');
  await expect(out).toContainText(expectedNpyBase64, { timeout: 15000 });
});

test('wav-to-numpy-npy supports deep-linked info report and non-default checkbox', async ({ page }) => {
  const params = new URLSearchParams({
    input: wavBase64,
    output: 'info',
    dtype: 'auto',
    shape: 'frames_channels',
    mono: 'true',
    start_frame: '1',
    max_frames: '2',
  });
  await page.goto(`/tools/wav-to-numpy-npy/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('Source WAV', { timeout: 15000 });
  await expect(out).toContainText('sample rate     16000 Hz');
  await expect(out).toContainText('dtype           int16');
  await expect(out).toContainText('shape           (2, 1)');
  await expect(out).toContainText('channels kept   1');
});

test('wav-to-numpy-npy accepts hex input and output hex', async ({ page }) => {
  await page.goto('/tools/wav-to-numpy-npy/');
  await page.fill('#in-input', wavHex);
  await page.selectOption('#in-input_format', 'hex');
  await page.selectOption('#in-dtype', 'int16');
  await page.selectOption('#in-output', 'hex');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('934e554d5059', { timeout: 15000 });
  await expect(out).toContainText('004000e00000');
});

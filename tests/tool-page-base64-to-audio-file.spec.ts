import { test, expect } from './fixtures';

const WAV_B64 = 'UklGRiwAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQgAAACAQICAgH9/fw==';
const PNG_B64 = 'iVBORw0KGgo=';

test('base64-to-audio-file page decodes WAV base64 to an audio data URL', async ({ page }) => {
  await page.goto('/tools/base64-to-audio-file/');
  await page.fill('#in-data', WAV_B64);
  await page.fill('#in-filename', 'beep');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('data:audio/wav;base64,', { timeout: 15000 });
});

test('base64-to-audio-file page deep-link accepts a data URI and forced hex-safe format', async ({ page }) => {
  const params = new URLSearchParams({
    data: `data:audio/wav;base64,${WAV_B64}`,
    filename: 'clip',
    format: 'wav',
    strict: 'true',
  });
  await page.goto(`/tools/base64-to-audio-file/?${params.toString()}`);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('data:audio/wav;base64,', { timeout: 15000 });
});

test('base64-to-audio-file page can save non-audio bytes when strict is unchecked', async ({ page }) => {
  await page.goto('/tools/base64-to-audio-file/');
  await page.fill('#in-data', PNG_B64);
  await page.uncheck('#in-strict');
  const out = page.locator('#tool-output');
  await expect(out).toContainText('data:application/octet-stream;base64,', { timeout: 15000 });
});

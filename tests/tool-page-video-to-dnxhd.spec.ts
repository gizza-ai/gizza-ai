import { test, expect } from './fixtures';
import path from 'node:path';

function bytesFromDataUrl(src: string): Buffer {
  const m = src.match(/^data:([^;,]+);base64,(.*)$/);
  if (!m) throw new Error(`not a base64 data URL: ${src.slice(0, 80)}`);
  return Buffer.from(m[2], 'base64');
}

function asciiContains(buf: Buffer, token: string): boolean {
  return buf.includes(Buffer.from(token, 'ascii'));
}

// DNxHR is not browser-decodable, so correctness is asserted by parsing the
// produced container bytes for the QuickTime/MXF and DNx markers that ffmpeg writes.
test('video-to-dnxhd creates a default DNxHR SQ MOV download', async ({ page }) => {
  await page.goto('/tools/video-to-dnxhd/');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-profile')).toHaveValue('dnxhr_sq');
  await expect(page.locator('#in-container')).toHaveValue('mov');
  await expect(page.locator('#in-resolution')).toHaveValue('source');
  await expect(page.locator('#in-pixel_format')).toHaveValue('auto');
  await expect(page.locator('#in-audio')).toHaveValue('pcm16');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/pillarbox-320x180.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/quicktime/);

  const bytes = bytesFromDataUrl(src!);
  expect(bytes.length).toBeGreaterThan(10_000);
  expect(asciiContains(bytes, 'ftypqt')).toBeTruthy();
  expect(asciiContains(bytes, 'AVdh')).toBeTruthy();
});

test('video-to-dnxhd deep-link creates an MXF proxy with no audio', async ({ page }) => {
  await page.goto('/tools/video-to-dnxhd/?profile=dnxhr_lb&container=mxf&resolution=720p&pixel_format=auto&audio=none');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-profile')).toHaveValue('dnxhr_lb');
  await expect(page.locator('#in-container')).toHaveValue('mxf');
  await expect(page.locator('#in-resolution')).toHaveValue('720p');
  await expect(page.locator('#in-audio')).toHaveValue('none');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/letterbox-320x240.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  // Browsers often surface MXF as application/octet-stream even though the
  // bytes carry the SMPTE MXF key, so assert the payload marker below.
  expect(src).toMatch(/^data:(application\/mxf|application\/octet-stream)/);

  const bytes = bytesFromDataUrl(src!);
  expect(bytes.length).toBeGreaterThan(10_000);
  expect(bytes.subarray(0, 4).equals(Buffer.from([0x06, 0x0e, 0x2b, 0x34]))).toBeTruthy();
});

test('video-to-dnxhd validates explicit pixel format against DNxHR profile', async ({ page }) => {
  await page.goto('/tools/video-to-dnxhd/?profile=dnxhr_hqx&pixel_format=yuv422p10le');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-profile')).toHaveValue('dnxhr_hqx');
  await expect(page.locator('#in-pixel_format')).toHaveValue('yuv422p10le');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/pillarbox-320x180.mp4'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  const bytes = bytesFromDataUrl(src!);
  expect(asciiContains(bytes, 'AVdh')).toBeTruthy();
});

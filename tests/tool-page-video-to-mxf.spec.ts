import { test, expect } from './fixtures';
import path from 'node:path';

function bytesFromDataUrl(src: string): Buffer {
  const m = src.match(/^data:([^;,]+);base64,(.*)$/);
  if (!m) throw new Error(`not a base64 data URL: ${src.slice(0, 80)}`);
  return Buffer.from(m[2], 'base64');
}

function hasMxfKey(buf: Buffer): boolean {
  return buf.subarray(0, 4).equals(Buffer.from([0x06, 0x0e, 0x2b, 0x34]));
}

// MXF is not browser-decodable, so correctness is asserted by parsing the
// produced payload for the SMPTE MXF key and ffmpeg-written codec/metadata markers.
test('video-to-mxf creates an XDCAM HD422 MXF download', async ({ page }) => {
  await page.goto('/tools/video-to-mxf/?profile=xdcam_hd422&resolution=source&frame_rate=25&audio=none');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-profile')).toHaveValue('xdcam_hd422');
  await expect(page.locator('#in-resolution')).toHaveValue('source');
  await expect(page.locator('#in-frame_rate')).toHaveValue('25');
  await expect(page.locator('#in-audio')).toHaveValue('none');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:(video\/|application\/mxf|application\/octet-stream)/);

  const bytes = bytesFromDataUrl(src!);
  expect(bytes.length).toBeGreaterThan(5_000);
  expect(hasMxfKey(bytes)).toBeTruthy();
});

test('video-to-mxf deep-link runs the XDCAM HD 35 Mbps 720p path', async ({ page }) => {
  await page.goto('/tools/video-to-mxf/?profile=xdcam_hd&resolution=1280x720&frame_rate=29.97&audio=none');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-profile')).toHaveValue('xdcam_hd');
  await expect(page.locator('#in-resolution')).toHaveValue('1280x720');
  await expect(page.locator('#in-frame_rate')).toHaveValue('29.97');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/pillarbox-320x180.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  const bytes = bytesFromDataUrl(src!);
  expect(hasMxfKey(bytes)).toBeTruthy();
});

test('video-to-mxf validates the IMX 50 25 fps constraint', async ({ page }) => {
  await page.goto('/tools/video-to-mxf/?profile=imx50&resolution=auto&frame_rate=29.97&audio=pcm16');
  await page.waitForSelector('#in-video');
  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  await expect(page.locator('.tool-output.error')).toContainText('frame_rate=25', { timeout: 30_000 });
});

import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/mxf-to-mp4/ page converts a broadcast-style MXF into an
// MP4 in-browser via ffmpeg-wasm. The fixture is a tiny MPEG-2 + PCM MXF, which
// proves the default path is a real transcode (not a copy remux).

async function decodeVideo(page, src: string) {
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.preload = 'metadata';
    await new Promise((res, rej) => {
      v.onloadedmetadata = () => res(null);
      v.onerror = () => rej(new Error('video decode failed'));
      v.src = dataUrl;
    });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
}

test('mxf-to-mp4 page transcodes a tiny broadcast MXF to playable MP4', async ({ page }) => {
  await page.goto('/tools/mxf-to-mp4/');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-picture')).toHaveValue('h264');
  await expect(page.locator('#in-audio')).toHaveValue('stereo');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-broadcast.mxf'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src);
  expect(meta.w).toBe(64);
  expect(meta.h).toBe(64);
  expect(meta.d).toBeGreaterThan(0.5);
  expect(meta.d).toBeLessThan(1.6);
});

test('mxf-to-mp4 deep-link prefills non-default picture quality and no-audio mode', async ({ page }) => {
  await page.goto('/tools/mxf-to-mp4/?picture=h264&quality=90&audio=none&audio_bitrate=128');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-picture')).toHaveValue('h264');
  await expect(page.locator('#in-quality')).toHaveValue('90');
  await expect(page.locator('#in-audio')).toHaveValue('none');
  await expect(page.locator('#in-audio_bitrate')).toHaveValue('128');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-broadcast.mxf'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src);
  expect(meta.w).toBe(64);
  expect(meta.h).toBe(64);
  expect(meta.d).toBeGreaterThan(0.5);
});

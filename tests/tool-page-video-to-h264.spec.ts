import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-to-h264/ page force-transcodes an uploaded clip to
// universally-playable H.264 High-profile MP4 (yuv420p, +faststart, AAC) in the
// browser via ffmpeg-wasm (single-threaded @ffmpeg/core from jsDelivr — needs
// network). Output is ALWAYS a data:video/mp4 URL, regardless of input codec.
//
// Each test asserts REAL output correctness: it decodes the produced video in a
// <video> element and checks the container MIME + preserved dimensions/duration.
// The browser being able to decode+play the result at all is itself the proof
// that the output is H.264/MP4 (a browser <video> cannot play VP9-in-webm on
// Safari, an HEVC-in-mkv, etc. — only the normalized H.264/MP4 plays everywhere).
// The exact H.264 profile pinning (high/main/baseline + yuv420p) is asserted by
// the core unit tests (argv tokens) and verified natively via ffprobe.

// Decode a data:video/ URL and read its intrinsic size + duration.
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

test('video-to-h264 normalizes a VP9/Opus .webm to playable H.264 MP4 (default high profile)', async ({ page }) => {
  await page.goto('/tools/video-to-h264/');
  await page.waitForSelector('#in-video');
  // Default profile is "high", default quality 75. The webm case is the headline
  // "won't play in Safari -> plays everywhere" normalize.
  await expect(page.locator('#in-profile')).toHaveValue('high');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/clip-1s.webm'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/); // container is always MP4

  // A pure normalize keeps frame size + duration (no scale/trim). clip-1s.webm
  // is 64x64, ~1s. The browser decoding it at all proves it's H.264/MP4.
  const meta = await decodeVideo(page, src);
  expect(meta.w).toBe(64);
  expect(meta.h).toBe(64);
  expect(meta.d).toBeGreaterThan(0.5);
  expect(meta.d).toBeLessThan(1.6);
});

test('video-to-h264 deep-link re-encodes an MP4 at baseline profile, non-default quality', async ({ page }) => {
  // Deep-link prefills the profile select + quality field; the run fires on upload.
  // baseline = the non-default enum choice; quality=70 = a non-default value.
  await page.goto('/tools/video-to-h264/?profile=baseline&quality=70');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-profile')).toHaveValue('baseline');
  await expect(page.locator('#in-quality')).toHaveValue('70');

  // Secondary input format: a 128x128 H.264 .mp4 -> re-encode to baseline H.264.
  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(0.5);
});

test('video-to-h264 re-encodes a QuickTime .mov at main profile (tertiary format)', async ({ page }) => {
  await page.goto('/tools/video-to-h264/');
  await page.waitForSelector('#in-video');

  // Third enum choice (main) exercised via the UI select; third input format (.mov).
  await page.selectOption('#in-profile', 'main');
  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-h264.mov'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(1.5);
  expect(meta.d).toBeLessThan(2.6);
});

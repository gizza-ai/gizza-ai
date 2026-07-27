import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-audio-bitrate-set/ page re-encodes ONLY the audio
// track of an uploaded clip at a chosen constant bitrate (kbps), stream-copying
// the video untouched, in the browser via ffmpeg-wasm (single-threaded
// @ffmpeg/core from jsDelivr — needs network). Output keeps the input container
// (mp4 -> mp4), so the result is a data:video/mp4 URL.
//
// Each test asserts REAL output correctness: it decodes the produced video in a
// <video> element and checks the container MIME + preserved dimensions/duration.
// Because the video stream is stream-copied, the frame size and duration must
// survive intact — proof the picture was left alone while the audio was
// re-encoded. The exact `-b:a <rate>k` argv is asserted by the core unit tests.

// tiny-128x128-audio.mp4: 128x128 H.264 + AAC, ~2s.
const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128-audio.mp4');

// Decode a data:video/ URL and read its intrinsic size + duration.
async function decodeVideo(page, src: string) {
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.preload = 'metadata';
    await new Promise((res, rej) => {
      v.onloadedmetadata = () => res(null);
      v.onerror = () => rej(new Error('video-audio-bitrate-set output failed to decode'));
      v.src = dataUrl;
    });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
}

async function expectPlayableMp4(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/); // container is preserved (mp4 -> mp4)

  // Pure audio re-encode: the stream-copied picture keeps its size + duration.
  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(1.5);
  expect(meta.d).toBeLessThan(2.6);
}

test('video-audio-bitrate-set page re-encodes the audio at a non-default bitrate via the UI select', async ({ page }) => {
  await page.goto('/tools/video-audio-bitrate-set/');
  await page.waitForSelector('#in-file');
  // Default is 128; pick 96 (a non-default enum choice) through the select.
  await page.selectOption('#in-bitrate', '96');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableMp4(page);
});

test('video-audio-bitrate-set deep-link prefills the bitrate select and runs on upload', async ({ page }) => {
  // Deep-link prefills the bitrate select; the run fires on upload. 192 = a
  // non-default "keep music clean" choice.
  await page.goto('/tools/video-audio-bitrate-set/?bitrate=192');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-bitrate')).toHaveValue('192');

  await page.setInputFiles('#in-file', fixture);
  await expectPlayableMp4(page);
});

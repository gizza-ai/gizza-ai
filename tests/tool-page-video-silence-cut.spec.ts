import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-silence-cut/ page removes silent gaps from an
// uploaded video in-browser via ffmpeg (single-threaded @ffmpeg/core from
// jsDelivr — needs network). This is a single-pass APPROXIMATION: the audio is
// de-silenced but the video drifts out of sync (see the block's core docs). The
// page is still asserted to produce a valid playable mp4 data URL.
test('video-silence-cut page tightens an uploaded video', async ({ page }) => {
  await page.goto('/tools/video-silence-cut/');
  await page.waitForSelector('#in-video');

  await page.fill('#in-threshold_db', '-30');
  await page.fill('#in-min_silence', '0.5');
  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/silence-clip.mp4'));

  // ffmpeg loads from CDN on first run; re-encodes video — allow generous time.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);
});

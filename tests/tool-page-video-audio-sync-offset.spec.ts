import { test, expect } from './fixtures';
import path from 'node:path';

// tiny-av-128x128.mp4 has BOTH a video and an audio track — required, since the
// tool re-times the audio (a video-only clip would have no stream to shift).
const fixture = path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4');

async function expectSyncedVideoDataUrl(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  const frame = await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video-audio-sync-offset output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  // Picture is stream-copied, so dimensions are unchanged; a positive delay
  // pads the audio front, so the output is at least as long as the input.
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-audio-sync-offset page delays audio by a positive ms offset', async ({ page }) => {
  await page.goto('/tools/video-audio-sync-offset/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-offset', '200');
  await page.setInputFiles('#in-file', fixture);
  await expectSyncedVideoDataUrl(page);
});

test('video-audio-sync-offset page honors query params (advance audio in seconds)', async ({ page }) => {
  await page.goto('/tools/video-audio-sync-offset/?offset=-0.2&unit=seconds');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-offset')).toHaveValue('-0.2');
  await expect(page.locator('#in-unit')).toHaveValue('seconds');
  await page.setInputFiles('#in-file', fixture);
  await expectSyncedVideoDataUrl(page);
});

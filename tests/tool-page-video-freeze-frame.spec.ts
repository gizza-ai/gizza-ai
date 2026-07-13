import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/video-freeze-frame/ page runs ffmpeg in-browser
// (@ffmpeg/core from jsDelivr — needs network). It splits the clip at the freeze
// point, clones that frame for the hold duration (tpad), and concats it back into
// an H.264 video, so the output is exactly `duration` seconds LONGER than the
// source. The proof of a real output is a playable video data URL whose duration
// grew by roughly the hold length.
const FIXTURE = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function outputVideoDuration(page: Page): Promise<number> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);
  return await page.evaluate(
    (dataUrl: string) =>
      new Promise<number>((resolve, reject) => {
        const v = document.createElement('video');
        v.preload = 'metadata';
        v.onloadedmetadata = () => resolve(v.duration);
        v.onerror = () => reject(new Error('video failed to load'));
        v.src = dataUrl;
      }),
    src!,
  );
}

test('video-freeze-frame page holds a frame and lengthens the clip', async ({ page }) => {
  await page.goto('/tools/video-freeze-frame/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', FIXTURE);
  await page.fill('#in-time', '1');
  await page.fill('#in-duration', '2');
  const duration = await outputVideoDuration(page);
  // The fixture is ~1s; a 2s hold makes the output clearly longer than the source.
  expect(duration).toBeGreaterThan(2.5);
});

test('video-freeze-frame deep link honors time and duration params', async ({ page }) => {
  await page.goto('/tools/video-freeze-frame/?time=0&duration=3');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-time')).toHaveValue('0', { timeout: 15_000 });
  await expect(page.locator('#in-duration')).toHaveValue('3');
  await page.setInputFiles('#in-file', FIXTURE);
  const duration = await outputVideoDuration(page);
  // Holding the opening frame for 3s adds ~3s to the short fixture.
  expect(duration).toBeGreaterThan(3.5);
});

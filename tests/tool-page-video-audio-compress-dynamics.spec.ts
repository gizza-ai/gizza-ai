import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function expectPlayableVideoDataUrl(page) {
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
      v.addEventListener('error', () => reject(new Error('video-audio-compress-dynamics output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-audio-compress-dynamics page runs the default medium compressor', async ({ page }) => {
  await page.goto('/tools/video-audio-compress-dynamics/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-audio-compress-dynamics page honors deep-linked heavy preset without makeup', async ({ page }) => {
  await page.goto('/tools/video-audio-compress-dynamics/?preset=heavy&makeup=false');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-preset')).toHaveValue('heavy');
  await expect(page.locator('#in-makeup')).not.toBeChecked();
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

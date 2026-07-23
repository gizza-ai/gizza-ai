import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4');

async function expectFadedVideoDataUrl(page) {
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
      v.addEventListener('error', () => reject(new Error('video-audio-fade output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-audio-fade page fades audio in and out', async ({ page }) => {
  await page.goto('/tools/video-audio-fade/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-fade_in', '0.5');
  await page.fill('#in-fade_out', '0.5');
  await page.setInputFiles('#in-file', fixture);
  await expectFadedVideoDataUrl(page);
});

test('video-audio-fade page honors query params for fade-out only', async ({ page }) => {
  await page.goto('/tools/video-audio-fade/?fade_in=0&fade_out=0.5');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-fade_in')).toHaveValue('0');
  await expect(page.locator('#in-fade_out')).toHaveValue('0.5');
  await page.setInputFiles('#in-file', fixture);
  await expectFadedVideoDataUrl(page);
});

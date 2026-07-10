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
      v.addEventListener('error', () => reject(new Error('video-audio-denoise output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-audio-denoise page denoises with the default afftdn path', async ({ page }) => {
  await page.goto('/tools/video-audio-denoise/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-strength', '12');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-audio-denoise page honors query params, anlmdn, and remove-hum checkbox', async ({ page }) => {
  await page.goto('/tools/video-audio-denoise/?strength=25&method=anlmdn&remove_hum=true');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-strength')).toHaveValue('25');
  await expect(page.locator('#in-method')).toHaveValue('anlmdn');
  await expect(page.locator('#in-remove_hum')).toBeChecked();
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

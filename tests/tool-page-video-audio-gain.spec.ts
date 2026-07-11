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
      v.addEventListener('error', () => reject(new Error('video-audio-gain output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-audio-gain page boosts audio with the default dB limiter path', async ({ page }) => {
  await page.goto('/tools/video-audio-gain/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-amount', '6');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-audio-gain page honors query params and factor mode without limiter', async ({ page }) => {
  await page.goto('/tools/video-audio-gain/?amount=0.5&unit=factor&limiter=false');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-amount')).toHaveValue('0.5');
  await expect(page.locator('#in-unit')).toHaveValue('factor');
  await expect(page.locator('#in-limiter')).not.toBeChecked();
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

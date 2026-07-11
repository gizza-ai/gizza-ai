import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4');

async function expectPlayableMp4(page) {
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
      v.addEventListener('error', () => reject(new Error('video-cut-segments output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-cut-segments page keeps two uploaded windows', async ({ page }) => {
  await page.goto('/tools/video-cut-segments/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-segments', '0-0.25, 0.5-0.75');
  await page.selectOption('#in-mode', 'keep');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableMp4(page);
});

test('video-cut-segments page honors query params and remove mode', async ({ page }) => {
  await page.goto('/tools/video-cut-segments/?segments=0.25-0.5&mode=remove');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-segments')).toHaveValue('0.25-0.5');
  await expect(page.locator('#in-mode')).toHaveValue('remove');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableMp4(page);
});

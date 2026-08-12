import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/redblue-64.mp4');

async function outputVideoInfo(page: Page): Promise<{ src: string; duration: number; width: number; height: number }> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);
  const info = await page.evaluate(
    (dataUrl: string) =>
      new Promise<{ duration: number; width: number; height: number }>((resolve, reject) => {
        const v = document.createElement('video');
        v.preload = 'metadata';
        v.onloadedmetadata = () => resolve({ duration: v.duration, width: v.videoWidth, height: v.videoHeight });
        v.onerror = () => reject(new Error('video failed to load'));
        v.src = dataUrl;
      }),
    src!,
  );
  return { src: src!, ...info };
}

test('video-set-keyframe-interval page re-encodes a playable fixed-GOP MP4', async ({ page }) => {
  await page.goto('/tools/video-set-keyframe-interval/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', FIXTURE);
  await page.fill('#in-interval', '1');
  await page.selectOption('#in-unit', 'seconds');
  await page.fill('#in-quality', '35');
  await page.selectOption('#in-preset', 'ultrafast');
  const info = await outputVideoInfo(page);
  expect(info.width).toBe(64);
  expect(info.height).toBe(64);
  expect(info.duration).toBeGreaterThan(1.5);
});

test('video-set-keyframe-interval deep link supports frame units and scene cuts', async ({ page }) => {
  await page.goto('/tools/video-set-keyframe-interval/?interval=10&unit=frames&scene_cut=true&closed_gop=false&quality=30&preset=veryfast');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-interval')).toHaveValue('10', { timeout: 15_000 });
  await expect(page.locator('#in-unit')).toHaveValue('frames');
  await expect(page.locator('#in-scene_cut')).toBeChecked();
  await expect(page.locator('#in-closed_gop')).not.toBeChecked();
  await expect(page.locator('#in-quality')).toHaveValue('30');
  await expect(page.locator('#in-preset')).toHaveValue('veryfast');
  await page.setInputFiles('#in-file', FIXTURE);
  const info = await outputVideoInfo(page);
  expect(info.src).toContain('data:video/mp4');
  expect(info.width).toBe(64);
  expect(info.height).toBe(64);
});

import { test, expect } from './fixtures';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4');

async function decodeVideo(page, src: string): Promise<{ w: number; h: number; d: number }> {
  return page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.preload = 'metadata';
    await new Promise<void>((res, rej) => {
      v.onloadedmetadata = () => res();
      v.onerror = () => rej(new Error('video decode failed'));
      v.src = dataUrl;
    });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
}

test('video-aspect-ratio-fix stream-copies a clip with the default 16:9 DAR tag', async ({ page }) => {
  await page.goto('/tools/video-aspect-ratio-fix/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-aspect')).toHaveValue('16:9');
  await expect(page.locator('#in-container')).toHaveValue('keep');
  await expect(page.locator('#in-faststart')).toBeChecked();

  await page.setInputFiles('#in-file', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  // The tool retags display metadata only: browsers expose the display size
  // after applying the new DAR (128px high at 16:9 -> ~228px wide) while the
  // stream-copy argv is asserted by core tests.
  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(228);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(0.5);
});

test('video-aspect-ratio-fix deep link accepts custom decimal ratios and a non-default checkbox', async ({ page }) => {
  await page.goto('/tools/video-aspect-ratio-fix/?aspect=custom&custom_aspect=1.85&container=mp4&faststart=false');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-aspect')).toHaveValue('custom', { timeout: 15_000 });
  await expect(page.locator('#in-custom_aspect')).toHaveValue('1.85');
  await expect(page.locator('#in-container')).toHaveValue('mp4');
  await expect(page.locator('#in-faststart')).not.toBeChecked();

  await page.setInputFiles('#in-file', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(237);
  expect(meta.h).toBe(128);
});

test('video-aspect-ratio-fix page ships runnable CLI and labeled preset controls', async ({ page }) => {
  await page.goto('/tools/video-aspect-ratio-fix/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(
    "gizza tool video-aspect-ratio-fix 'url=https://example.com/input' 'aspect=16:9' 'custom_aspect=1.85' 'container=keep' 'faststart=true'"
  );
  await expect(page.locator('#in-aspect option[value="9:16"]')).toHaveText('9:16 — vertical (Reels, Shorts, TikTok)');
  await expect(page.locator('#in-aspect option[value="2.39:1"]')).toHaveText('2.39:1 — cinemascope');
  await expect(page.locator('#in-container option[value="mkv"]')).toHaveText('mkv — most codec-tolerant');
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
});

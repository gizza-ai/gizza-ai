import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(page, container: string, faststart: string, regenTimestamps: string, inName: string) {
  return await page.evaluate(async ({ container, faststart, regenTimestamps, inName }) => {
    const mod = await import('/tools/video-duration-fix-remux/gizza_ai_video_duration_fix_remux_web.js');
    await mod.default('/tools/video-duration-fix-remux/gizza_ai_video_duration_fix_remux_web_bg.wasm');
    return mod.build_argv(container, faststart, regenTimestamps, inName);
  }, { container, faststart, regenTimestamps, inName });
}

async function decodeVideo(page, src: string) {
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.preload = 'metadata';
    await new Promise((res, rej) => {
      v.onloadedmetadata = () => res(null);
      v.onerror = () => rej(new Error('video-duration-fix-remux output failed to decode'));
      v.src = dataUrl;
    });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
}

test('video-duration-fix-remux wasm build_argv builds exact keep+faststart remux plan', async ({ page }) => {
  await page.goto('/tools/video-duration-fix-remux/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, 'keep', 'true', 'false', 'in.mp4');
  expect(plan.out_name).toBe('out.mp4');
  expect(plan.argv).toEqual(['-i', 'in.mp4', '-map', '0', '-c', 'copy', '-movflags', '+faststart', 'out.mp4']);
});

test('video-duration-fix-remux page remuxes an MP4 without re-encoding', async ({ page }) => {
  await page.goto('/tools/video-duration-fix-remux/');
  await page.waitForSelector('#in-file');

  await page.setInputFiles('#in-file', fixture);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(0);
});

test('video-duration-fix-remux deep-link prefills container and timestamp flags', async ({ page }) => {
  await page.goto('/tools/video-duration-fix-remux/?container=mkv&faststart=false&regen_timestamps=true');
  await page.waitForSelector('#in-file');

  await expect(page.locator('#in-container')).toHaveValue('mkv');
  await expect(page.locator('#in-faststart')).not.toBeChecked();
  await expect(page.locator('#in-regen_timestamps')).toBeChecked();

  const plan = await buildArgv(page, 'mkv', 'false', 'true', 'broken.webm');
  expect(plan.out_name).toBe('out.mkv');
  expect(plan.argv).toEqual(['-fflags', '+genpts', '-i', 'broken.webm', '-map', '0', '-c', 'copy', 'out.mkv']);
});

import { test, expect } from './fixtures';
import path from 'node:path';

// /tools/video-timelapse/ speeds video up with setpts, re-samples with fps,
// drops audio, and re-encodes to H.264 in-browser via ffmpeg. The shared wasm
// build_argv export is also asserted directly so argv drift is caught cheaply.
const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(page, speed: number, fps: number, inName: string) {
  return await page.evaluate(
    async ({ speed, fps, inName }) => {
      const mod = await import('/tools/video-timelapse/gizza_ai_video_timelapse_web.js');
      await mod.default('/tools/video-timelapse/gizza_ai_video_timelapse_web_bg.wasm');
      return mod.build_argv(speed, fps, inName);
    },
    { speed, fps, inName },
  );
}

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
      v.addEventListener('error', () => reject(new Error('video-timelapse output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-timelapse page produces a playable silent mp4', async ({ page }) => {
  await page.goto('/tools/video-timelapse/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-speed', '4');
  await page.fill('#in-fps', '12');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-timelapse page honors ?speed=&fps= deep link', async ({ page }) => {
  await page.goto('/tools/video-timelapse/?speed=20&fps=24');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-speed')).toHaveValue('20');
  await expect(page.locator('#in-fps')).toHaveValue('24');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-timelapse wasm build_argv builds the expected timelapse plan', async ({ page }) => {
  await page.goto('/tools/video-timelapse/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, 10, 30, 'in.mp4');
  expect(plan.out_name).toBe('out.mp4');
  expect(plan.argv[plan.argv.indexOf('-vf') + 1]).toBe('setpts=PTS/10,fps=30');
  expect(plan.argv).toContain('-an');
  expect(plan.argv).toContain('libx264');
  expect(plan.argv).toContain('yuv420p');

  const cinematic = await buildArgv(page, 60, 24, 'clip.mov');
  expect(cinematic.out_name).toBe('out.mov');
  expect(cinematic.argv[cinematic.argv.indexOf('-vf') + 1]).toBe('setpts=PTS/60,fps=24');

  const webm = await buildArgv(page, 8, 29.97, 'clip.webm');
  expect(webm.out_name).toBe('out.mp4');
  expect(webm.argv[webm.argv.indexOf('-vf') + 1]).toBe('setpts=PTS/8,fps=29.97');

  const clamped = await buildArgv(page, 1000, 120, 'in.mp4');
  expect(clamped.argv[clamped.argv.indexOf('-vf') + 1]).toBe('setpts=PTS/300,fps=60');
});

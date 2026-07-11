import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(page, method: string, strength: number, inName: string) {
  return await page.evaluate(async ({ method, strength, inName }) => {
    const mod = await import('/tools/video-denoise/gizza_ai_video_denoise_web.js');
    await mod.default('/tools/video-denoise/gizza_ai_video_denoise_web_bg.wasm');
    return mod.build_argv(method, strength, inName);
  }, { method, strength, inName });
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
      v.addEventListener('error', () => reject(new Error('video-denoise output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-denoise page denoises with the default hqdn3d path', async ({ page }) => {
  await page.goto('/tools/video-denoise/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-method')).toHaveValue('hqdn3d');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-denoise page honors ?method=nlmeans&strength=40 deep link', async ({ page }) => {
  await page.goto('/tools/video-denoise/?method=nlmeans&strength=40');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-method')).toHaveValue('nlmeans');
  await expect(page.locator('#in-strength')).toHaveValue('40');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-denoise wasm build_argv builds the expected hqdn3d and nlmeans plans', async ({ page }) => {
  await page.goto('/tools/video-denoise/');
  await page.waitForSelector('#in-file');

  // Default hqdn3d at strength 30 → ls=3, cs=2.25, lt=4.5, ct=3.375; H.264 re-encode.
  const hq = await buildArgv(page, 'hqdn3d', 30, 'in.mp4');
  expect(hq.out_name).toBe('out.mp4');
  expect(hq.argv).toContain('-vf');
  expect(hq.argv).toContain('libx264');
  expect(hq.argv[hq.argv.indexOf('-vf') + 1]).toBe('hqdn3d=3:2.25:4.5:3.375');
  expect(hq.argv).toContain('copy'); // audio stream-copied for mp4

  // nlmeans at strength 40 → s = 1 + 39·14/99 = 6.51515…
  const nl = await buildArgv(page, 'nlmeans', 40, 'in.mp4');
  expect(nl.out_name).toBe('out.mp4');
  expect(nl.argv[nl.argv.indexOf('-vf') + 1]).toBe('nlmeans=s=6.51515');
  expect(nl.argv).toContain('libx264');
});

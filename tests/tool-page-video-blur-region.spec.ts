import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-blur-region/ page blurs or pixelates a fixed
// rectangle on every frame of an uploaded video in-browser via ffmpeg
// (@ffmpeg/core from jsDelivr — needs network). Output keeps the mp4 container,
// so the media src is a data:video/mp4 URL. The wasm `build_argv` is pure and
// shared with the chat block via core, so we also assert the argv/out-name plan
// directly the same way the nearby ffmpeg page specs do.
const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(
  page,
  x: number,
  y: number,
  width: number,
  height: number,
  mode: string,
  strength: number,
  inName: string,
) {
  return await page.evaluate(
    async ({ x, y, width, height, mode, strength, inName }) => {
      const mod = await import('/tools/video-blur-region/gizza_ai_video_blur_region_web.js');
      await mod.default('/tools/video-blur-region/gizza_ai_video_blur_region_web_bg.wasm');
      return mod.build_argv(x, y, width, height, mode, strength, inName);
    },
    { x, y, width, height, mode, strength, inName },
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
      v.addEventListener('error', () => reject(new Error('video-blur-region output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-blur-region page blurs a region with the default mode', async ({ page }) => {
  await page.goto('/tools/video-blur-region/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-mode')).toHaveValue('blur');

  await page.fill('#in-x', '10');
  await page.fill('#in-y', '10');
  await page.fill('#in-width', '64');
  await page.fill('#in-height', '64');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-blur-region page honors ?x=&y=&width=&height=&mode=pixelate&strength deep link', async ({ page }) => {
  await page.goto('/tools/video-blur-region/?x=16&y=24&width=80&height=48&mode=pixelate&strength=16');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-x')).toHaveValue('16');
  await expect(page.locator('#in-y')).toHaveValue('24');
  await expect(page.locator('#in-width')).toHaveValue('80');
  await expect(page.locator('#in-height')).toHaveValue('48');
  await expect(page.locator('#in-mode')).toHaveValue('pixelate');
  await expect(page.locator('#in-strength')).toHaveValue('16');

  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-blur-region wasm build_argv builds the expected blur and pixelate plans', async ({ page }) => {
  await page.goto('/tools/video-blur-region/');
  await page.waitForSelector('#in-file');

  // Blur → crop the region, gblur=sigma=strength, overlay back; H.264 re-encode.
  const blur = await buildArgv(page, 10, 20, 320, 240, 'blur', 25, 'in.mp4');
  expect(blur.out_name).toBe('out.mp4');
  expect(blur.argv).toContain('-filter_complex');
  expect(blur.argv).toContain('libx264');
  expect(blur.argv[blur.argv.indexOf('-filter_complex') + 1]).toBe(
    '[0:v]crop=320:240:10:20,gblur=sigma=25[fg];[0:v][fg]overlay=10:20',
  );

  // Pixelate at block size 16 → 320/16=20, 160/16=10 downscale + neighbor upscale.
  const pix = await buildArgv(page, 0, 0, 320, 160, 'pixelate', 16, 'in.mp4');
  expect(pix.out_name).toBe('out.mp4');
  expect(pix.argv[pix.argv.indexOf('-filter_complex') + 1]).toBe(
    '[0:v]crop=320:160:0:0,scale=20:10:flags=neighbor,scale=320:160:flags=neighbor[fg];[0:v][fg]overlay=0:0',
  );
  expect(pix.argv).toContain('libx264');

  // Empty mode falls back to blur; non-positive strength falls back to core default 20.
  const dflt = await buildArgv(page, 5, 5, 100, 100, '', 0, 'clip.webm');
  expect(dflt.out_name).toBe('out.mp4'); // webm switches to mp4
  expect(dflt.argv[dflt.argv.indexOf('-filter_complex') + 1]).toBe(
    '[0:v]crop=100:100:5:5,gblur=sigma=20[fg];[0:v][fg]overlay=5:5',
  );
});

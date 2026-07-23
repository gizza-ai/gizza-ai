import { test, expect } from './fixtures';
import path from 'node:path';

// A single still photo is the upload; the output size is chosen by the
// width/height fields, independent of the source, so we can assert exact dims.
const fixture = path.resolve(__dirname, 'fixtures/photo-320.jpg');

async function buildArgv(
  page,
  direction: string,
  duration: number,
  zoom: number,
  width: number,
  height: number,
  fps: number,
  inName: string,
) {
  return await page.evaluate(
    async ({ direction, duration, zoom, width, height, fps, inName }) => {
      const mod = await import('/tools/video-ken-burns/gizza_ai_video_ken_burns_web.js');
      await mod.default('/tools/video-ken-burns/gizza_ai_video_ken_burns_web_bg.wasm');
      return mod.build_argv(direction, duration, zoom, width, height, fps, inName);
    },
    { direction, duration, zoom, width, height, fps, inName },
  );
}

async function decodeOutput(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video-ken-burns output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
}

test('video-ken-burns page animates a photo with the default zoom-in move', async ({ page }) => {
  await page.goto('/tools/video-ken-burns/');
  await page.waitForSelector('#in-file');
  // The motion select defaults to zoom-in.
  await expect(page.locator('#in-direction')).toHaveValue('zoom-in');
  // Small explicit size + short duration keep the browser ffmpeg run fast.
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '240');
  await page.fill('#in-duration', '2');
  await page.fill('#in-fps', '24');
  await page.setInputFiles('#in-file', fixture);
  const clip = await decodeOutput(page);
  expect(clip.w).toBe(320);
  expect(clip.h).toBe(240);
  // -t 2 at 24fps → ~2 s of real, decodable MP4.
  expect(clip.duration).toBeGreaterThan(1.5);
  expect(clip.duration).toBeLessThan(2.6);
});

test('video-ken-burns page honors a ?direction=pan-right deep link and its exact size', async ({ page }) => {
  await page.goto('/tools/video-ken-burns/?direction=pan-right&zoom=1.4&duration=2&width=426&height=240&fps=24');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-direction')).toHaveValue('pan-right');
  await expect(page.locator('#in-width')).toHaveValue('426');
  await expect(page.locator('#in-zoom')).toHaveValue('1.4');
  await page.setInputFiles('#in-file', fixture);
  const clip = await decodeOutput(page);
  // Exact output dims prove the scale→crop→zoompan chain actually ran in ffmpeg.
  expect(clip.w).toBe(426);
  expect(clip.h).toBe(240);
  expect(clip.duration).toBeGreaterThan(1.5);
  expect(clip.duration).toBeLessThan(2.6);
});

test('video-ken-burns page renders a zoom-out vertical story clip', async ({ page }) => {
  await page.goto('/tools/video-ken-burns/?direction=zoom-out&duration=2&zoom=1.5&width=240&height=426&fps=24');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-direction')).toHaveValue('zoom-out');
  await page.setInputFiles('#in-file', fixture);
  const clip = await decodeOutput(page);
  // Vertical (portrait) output — width < height.
  expect(clip.w).toBe(240);
  expect(clip.h).toBe(426);
  expect(clip.duration).toBeGreaterThan(1.5);
  expect(clip.duration).toBeLessThan(2.6);
});

test('video-ken-burns wasm build_argv covers defaults, moves and validation', async ({ page }) => {
  await page.goto('/tools/video-ken-burns/');
  await page.waitForSelector('#in-file');

  // Empty/0 numeric fields fall back to the defaults (1280x720, 5s, 30fps, zoom 1.3).
  const def = await buildArgv(page, '', 0, 0, 0, 0, 0, 'in.png');
  expect(def.out_name).toBe('out.mp4');
  const dvf = def.argv[def.argv.indexOf('-vf') + 1];
  expect(dvf).toContain('s=1280x720');
  expect(dvf).toContain('fps=30');
  expect(dvf).toContain("z='1+0.3*on/149'"); // 5s*30 = 150 frames → den 149
  expect(def.argv).toContain('libx264');
  expect(def.argv).toContain('yuv420p');
  expect(def.argv[def.argv.indexOf('-t') + 1]).toBe('5');

  // Zoom-out starts zoomed in and eases back.
  const out = await buildArgv(page, 'zoom-out', 1, 1.5, 640, 480, 25, 'in.jpg');
  const ovf = out.argv[out.argv.indexOf('-vf') + 1];
  expect(ovf).toContain("z='1.5-0.5*on/24'");
  expect(ovf).toContain('s=640x480');

  // Pan-right holds the zoom and slides x across the frame.
  const pan = await buildArgv(page, 'pan-right', 2, 1.4, 800, 600, 30, 'in.png');
  const pvf = pan.argv[pan.argv.indexOf('-vf') + 1];
  expect(pvf).toContain("z='1.4'");
  expect(pvf).toContain("x='(iw-iw/zoom)*on/59'");

  // Vertical story dimensions round-trip through the plan.
  const vert = await buildArgv(page, 'zoom-in', 3, 1.3, 1080, 1920, 30, 'in.png');
  expect(vert.argv[vert.argv.indexOf('-vf') + 1]).toContain('s=1080x1920');

  // Odd dims snap down to even (H.264 requirement).
  const odd = await buildArgv(page, 'zoom-in', 2, 1.2, 641, 481, 30, 'in.png');
  expect(odd.argv[odd.argv.indexOf('-vf') + 1]).toContain('s=640x480');

  // Unknown motion throws the descriptive core error.
  await expect(buildArgv(page, 'spin', 2, 1.3, 640, 480, 30, 'in.png')).rejects.toThrow(/invalid direction/);
});

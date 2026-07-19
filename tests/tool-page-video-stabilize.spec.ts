import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(page, borders: string, strength: number, inName: string) {
  return await page.evaluate(async ({ borders, strength, inName }) => {
    const mod = await import('/tools/video-stabilize/gizza_ai_video_stabilize_web.js');
    await mod.default('/tools/video-stabilize/gizza_ai_video_stabilize_web_bg.wasm');
    return mod.build_argv(borders, strength, inName);
  }, { borders, strength, inName });
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
      v.addEventListener('error', () => reject(new Error('video-stabilize output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
}

test('video-stabilize page stabilizes with the default mirror path', async ({ page }) => {
  await page.goto('/tools/video-stabilize/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-borders')).toHaveValue('mirror');
  await page.setInputFiles('#in-file', fixture);
  const frame = await decodeOutput(page);
  // Mirror mode keeps the full original resolution.
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
});

test('video-stabilize page honors ?borders=crop&strength=60 deep link and crop-zooms', async ({ page }) => {
  await page.goto('/tools/video-stabilize/?borders=crop&strength=60');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-borders')).toHaveValue('crop');
  await expect(page.locator('#in-strength')).toHaveValue('60');
  await page.setInputFiles('#in-file', fixture);
  const frame = await decodeOutput(page);
  // Crop & zoom at strength 60 → z = 1.068: 128 → scale trunc(128*1.068/2)*2
  // = 136 → crop trunc(136/1.068/2)*2 = 126. Exact, so this proves the
  // deshake+scale+crop chain actually ran in the browser ffmpeg build.
  expect(frame.w).toBe(126);
  expect(frame.h).toBe(126);
  expect(frame.duration).toBeGreaterThan(0);
});

test('video-stabilize wasm build_argv covers every borders mode and the radius snapping', async ({ page }) => {
  await page.goto('/tools/video-stabilize/');
  await page.waitForSelector('#in-file');

  // Default: empty borders → mirror, strength 0 sentinel → 25 → rx/ry 16.
  const def = await buildArgv(page, '', 0, 'in.mp4');
  expect(def.out_name).toBe('out.mp4');
  expect(def.argv[def.argv.indexOf('-vf') + 1]).toBe('deshake=rx=16:ry=16:edge=mirror');
  expect(def.argv).toContain('libx264');
  expect(def.argv).toContain('copy'); // audio stream-copied for mp4

  // blank at strength 100 → the 64 px radius cap.
  const blank = await buildArgv(page, 'blank', 100, 'in.mp4');
  expect(blank.argv[blank.argv.indexOf('-vf') + 1]).toBe('deshake=rx=64:ry=64:edge=blank');

  // original at strength 26 → snaps up to the 32 px step.
  const orig = await buildArgv(page, 'original', 26, 'in.mp4');
  expect(orig.argv[orig.argv.indexOf('-vf') + 1]).toBe('deshake=rx=32:ry=32:edge=original');

  // crop at strength 60 → zoom chain with z = 1.068.
  const crop = await buildArgv(page, 'crop', 60, 'in.mp4');
  expect(crop.argv[crop.argv.indexOf('-vf') + 1]).toBe(
    'deshake=rx=48:ry=48:edge=blank,scale=trunc(iw*1.068/2)*2:trunc(ih*1.068/2)*2,crop=trunc(iw/1.068/2)*2:trunc(ih/1.068/2)*2'
  );

  // webm input switches container to mp4 and re-encodes audio to AAC.
  const webm = await buildArgv(page, 'mirror', 30, 'clip.webm');
  expect(webm.out_name).toBe('out.mp4');
  expect(webm.argv).toContain('aac');

  // Invalid borders throws the descriptive core error.
  await expect(buildArgv(page, 'zoom', 30, 'in.mp4')).rejects.toThrow(/not supported/);
});

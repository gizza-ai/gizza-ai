import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny.gif');

async function buildArgv(
  page,
  sensitivity: number,
  timing: string,
  maxFps: number,
  format: string,
  frac: number,
  inName: string,
) {
  return await page.evaluate(
    async ({ sensitivity, timing, maxFps, format, frac, inName }) => {
      const mod = await import('/tools/video-dedup-frames/gizza_ai_video_dedup_frames_web.js');
      await mod.default('/tools/video-dedup-frames/gizza_ai_video_dedup_frames_web_bg.wasm');
      return mod.build_argv(sensitivity, timing, maxFps, format, frac, inName);
    },
    { sensitivity, timing, maxFps, format, frac, inName },
  );
}

test('video-dedup-frames page creates a real MP4 output', async ({ page }) => {
  await page.goto('/tools/video-dedup-frames/?sensitivity=70&timing=keep&max_fps=12&format=mp4&frac=0.5');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-sensitivity')).toHaveValue('70');
  await expect(page.locator('#in-timing')).toHaveValue('keep');
  await expect(page.locator('#in-max_fps')).toHaveValue('12');
  await expect(page.locator('#in-format')).toHaveValue('mp4');
  await expect(page.locator('#in-frac')).toHaveValue('0.5');

  await page.setInputFiles('#in-file', fixture);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const decoded = await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl!;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadedmetadata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('dedup output did not decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src);
  expect(decoded.w).toBe(64);
  expect(decoded.h).toBe(64);
  expect(decoded.duration).toBeGreaterThan(0);
});

test('video-dedup-frames wasm build_argv covers timing, format, caps, and validation', async ({ page }) => {
  await page.goto('/tools/video-dedup-frames/');
  await page.waitForSelector('#in-file');

  const mp4 = await buildArgv(page, 70, 'keep', 12, 'mp4', 0.5, 'in.gif');
  expect(mp4.out_name).toBe('out.mp4');
  expect(mp4.argv).toContain('libx264');
  expect(mp4.argv).toContain('aac');
  expect(mp4.argv[mp4.argv.indexOf('-vf') + 1]).toBe('fps=12,mpdecimate=hi=1075:lo=448:frac=0.5');
  expect(mp4.argv[mp4.argv.indexOf('-fps_mode') + 1]).toBe('vfr');

  const compact = await buildArgv(page, 50, 'compact', 0, 'webm', 0.33, 'clip.mp4');
  expect(compact.out_name).toBe('out.webm');
  expect(compact.argv).toContain('libvpx-vp9');
  expect(compact.argv).toContain('-an');
  expect(compact.argv[compact.argv.indexOf('-vf') + 1]).toContain('setpts=N/FRAME_RATE/TB');

  const constant = await buildArgv(page, 50, 'constant', 30, 'auto', 0, 'screen.mp4');
  expect(constant.out_name).toBe('out.mp4');
  expect(constant.argv[constant.argv.indexOf('-fps_mode') + 1]).toBe('cfr');
  expect(constant.argv[constant.argv.indexOf('-vf') + 1]).toContain('fps=30,mpdecimate');

  await expect(buildArgv(page, Number.NaN, 'weird', 0, 'auto', 0.33, 'in.mp4')).rejects.toThrow(/timing/);
  await expect(buildArgv(page, 50, 'keep', 0, 'avi', 0.33, 'in.mp4')).rejects.toThrow(/format/);
  await expect(buildArgv(page, 50, 'keep', 0, 'auto', 2, 'in.mp4')).rejects.toThrow(/frac/);
});

import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny.gif');

async function buildArgv(
  page,
  radius: number,
  radiusUnit: string,
  corners: string,
  background: string,
  format: string,
  quality: number,
  keepAudio: string,
  inName: string,
) {
  return await page.evaluate(
    async ({ radius, radiusUnit, corners, background, format, quality, keepAudio, inName }) => {
      const mod = await import('/tools/video-rounded-corners/gizza_ai_video_rounded_corners_web.js');
      await mod.default('/tools/video-rounded-corners/gizza_ai_video_rounded_corners_web_bg.wasm');
      return mod.build_argv(radius, radiusUnit, corners, background, format, quality, keepAudio, inName);
    },
    { radius, radiusUnit, corners, background, format, quality, keepAudio, inName },
  );
}

async function expectRoundedMp4WithDarkCorner(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 240_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const frame = await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadedmetadata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video-rounded-corners output failed to decode')), { once: true });
    });
    v.currentTime = Math.min(0.1, Math.max(0, (v.duration || 1) / 2));
    await new Promise((resolve, reject) => {
      v.addEventListener('seeked', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video-rounded-corners output failed while seeking')), { once: true });
    });
    const canvas = document.createElement('canvas');
    canvas.width = v.videoWidth;
    canvas.height = v.videoHeight;
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(v, 0, 0);
    const corner = Array.from(ctx.getImageData(0, 0, 1, 1).data);
    const center = Array.from(ctx.getImageData(Math.floor(v.videoWidth / 2), Math.floor(v.videoHeight / 2), 1, 1).data);
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration, corner, center };
  }, src!);

  expect(frame.w).toBe(64);
  expect(frame.h).toBe(64);
  expect(frame.duration).toBeGreaterThan(0);
  expect(frame.corner[0]).toBeLessThan(50);
  expect(frame.corner[1]).toBeLessThan(50);
  expect(frame.corner[2]).toBeLessThan(50);
  expect(frame.center[0]).toBeLessThan(60);
  expect(frame.center[1]).toBeLessThan(60);
  expect(frame.center[2]).toBeGreaterThan(180);
}

test('video-rounded-corners page creates a black-background MP4 with rounded pixels from a deep link', async ({ page }) => {
  await page.goto('/tools/video-rounded-corners/?radius=32&radius_unit=px&corners=all&background=%23000000&format=mp4&quality=75&keep_audio=false');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-radius')).toHaveValue('32');
  await expect(page.locator('#in-background')).toHaveValue('#000000');
  await expect(page.locator('#in-format')).toHaveValue('mp4');
  await expect(page.locator('#in-keep_audio')).not.toBeChecked();
  await page.setInputFiles('#in-file', fixture);
  await expectRoundedMp4WithDarkCorner(page);
});

test('video-rounded-corners deep-link prefills percent top-corner settings', async ({ page }) => {
  await page.goto('/tools/video-rounded-corners/?radius=12&radius_unit=percent&corners=top&background=%23111827&format=mp4&quality=85&keep_audio=false');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-radius')).toHaveValue('12');
  await expect(page.locator('#in-radius_unit')).toHaveValue('percent');
  await expect(page.locator('#in-corners')).toHaveValue('top');
  await expect(page.locator('#in-background')).toHaveValue('#111827');
  await expect(page.locator('#in-format')).toHaveValue('mp4');
  await expect(page.locator('#in-quality')).toHaveValue('85');
  await expect(page.locator('#in-keep_audio')).not.toBeChecked();
});

test('video-rounded-corners wasm build_argv covers formats and validation', async ({ page }) => {
  await page.goto('/tools/video-rounded-corners/');
  await page.waitForSelector('#in-file');

  const mp4 = await buildArgv(page, 40, 'px', 'all', '#000000', 'mp4', 75, 'false', 'in.mp4');
  expect(mp4.out_name).toBe('out.mp4');
  expect(mp4.argv).toContain('libx264');
  expect(mp4.argv).toContain('-an');
  expect(mp4.argv[mp4.argv.indexOf('-filter_complex') + 1]).toContain('drawbox=x=0:y=0:w=iw:h=ih:color=0x000000:t=fill');

  const webm = await buildArgv(page, 50, 'percent', 'top', 'transparent', 'webm', 80, 'true', 'clip.mp4');
  expect(webm.out_name).toBe('out.webm');
  expect(webm.argv).toContain('libvpx-vp9');
  expect(webm.argv[webm.argv.indexOf('-filter_complex') + 1]).toContain('0.5*min(W,H)');

  await expect(buildArgv(page, 24, 'px', 'all', 'transparent', 'mp4', 75, 'true', 'in.mp4')).rejects.toThrow(/cannot store transparency/);
  await expect(buildArgv(page, 1001, 'px', 'all', 'transparent', 'webm', 75, 'true', 'in.mp4')).rejects.toThrow(/1-1000 px/);
});

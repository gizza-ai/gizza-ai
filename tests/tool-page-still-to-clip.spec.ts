import { test, expect } from './fixtures';
import path from 'node:path';

const IMAGE = path.resolve(__dirname, 'fixtures/red-2x2.png');

async function buildArgv(
  page,
  duration: number,
  width: number,
  height: number,
  fit: string,
  background: string,
  fps: number,
  format: string,
  quality: number,
  inName: string,
) {
  return await page.evaluate(
    async ({ duration, width, height, fit, background, fps, format, quality, inName }) => {
      const mod = await import('/tools/still-to-clip/gizza_ai_still_to_clip_web.js');
      await mod.default('/tools/still-to-clip/gizza_ai_still_to_clip_web_bg.wasm');
      return mod.build_argv(duration, width, height, fit, background, fps, format, quality, inName);
    },
    { duration, width, height, fit, background, fps, format, quality, inName },
  );
}

async function videoStats(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadedmetadata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('still-to-clip output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
}

test('still-to-clip page encodes a static MP4 at the requested size and duration', async ({ page }) => {
  await page.goto('/tools/still-to-clip/');
  await page.waitForSelector('#in-file');

  await page.fill('#in-duration', '0.5');
  await page.fill('#in-width', '64');
  await page.fill('#in-height', '48');
  await page.selectOption('#in-fit', 'contain');
  await page.fill('#in-background', '#000000');
  await page.fill('#in-fps', '2');
  await page.selectOption('#in-format', 'mp4');
  await page.fill('#in-quality', '70');
  await page.setInputFiles('#in-file', IMAGE);

  const stats = await videoStats(page);
  expect(stats.w).toBe(64);
  expect(stats.h).toBe(48);
  expect(stats.duration).toBeGreaterThanOrEqual(0.45);
  expect(stats.duration).toBeLessThan(0.8);
});

test('still-to-clip deep link fills fields and can cover-crop', async ({ page }) => {
  await page.goto('/tools/still-to-clip/?duration=1&width=48&height=48&fit=cover&background=%23ffffff&fps=5&format=mp4&quality=60');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-duration')).toHaveValue('1');
  await expect(page.locator('#in-width')).toHaveValue('48');
  await expect(page.locator('#in-height')).toHaveValue('48');
  await expect(page.locator('#in-fit')).toHaveValue('cover');
  await expect(page.locator('#in-fps')).toHaveValue('5');
  await expect(page.locator('#in-format')).toHaveValue('mp4');
  await expect(page.locator('#in-quality')).toHaveValue('60');

  await page.setInputFiles('#in-file', IMAGE);
  const stats = await videoStats(page);
  expect(stats.w).toBe(48);
  expect(stats.h).toBe(48);
});

test('still-to-clip wasm build_argv covers formats, color normalization, presets and errors', async ({ page }) => {
  await page.goto('/tools/still-to-clip/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, 5, 1920, 1080, 'contain', '#fff', 30, 'mp4', 80, 'in.png');
  expect(plan.out_name).toBe('out.mp4');
  expect(plan.argv.slice(0, 4)).toEqual(['-loop', '1', '-framerate', '30']);
  expect(plan.argv[plan.argv.indexOf('-t') + 1]).toBe('5');
  expect(plan.argv[plan.argv.indexOf('-vf') + 1]).toContain('pad=1920:1080');
  expect(plan.argv[plan.argv.indexOf('-vf') + 1]).toContain('color=0xFFFFFF');
  expect(plan.argv).toContain('libx264');

  const webm = await buildArgv(page, 3, 1080, 1080, 'cover', 'black', 10, 'webm', 60, 'photo.jpg');
  expect(webm.out_name).toBe('out.webm');
  expect(webm.argv[webm.argv.indexOf('-vf') + 1]).toContain('crop=1080:1080');
  expect(webm.argv).toContain('libvpx-vp9');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(
    "gizza tool still-to-clip 'url=https://example.com/input' 'duration=5' 'width=1920' 'height=1080' 'fit=contain' 'fps=30' 'format=mp4' 'quality=80'",
  );

  await expect(buildArgv(page, 0.05, 320, 240, 'contain', 'black', 30, 'mp4', 80, 'in.png')).rejects.toThrow(/duration/);
  await expect(buildArgv(page, 5, 320, 240, 'contain', 'not-a-color', 30, 'mp4', 80, 'in.png')).rejects.toThrow(/color/);
});

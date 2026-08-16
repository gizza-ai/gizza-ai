import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

function bytesFromDataUrl(src: string): Buffer {
  const m = src.match(/^data:([^;,]+);base64,(.*)$/);
  if (!m) throw new Error(`not a base64 data URL: ${src.slice(0, 80)}`);
  return Buffer.from(m[2], 'base64');
}

async function imageInfo(page, src: string): Promise<{ w: number; h: number; bytes: number }> {
  return await page.evaluate(async (dataUrl) => {
    const img = new Image();
    img.src = dataUrl;
    await new Promise((resolve, reject) => {
      img.addEventListener('load', resolve, { once: true });
      img.addEventListener('error', () => reject(new Error('animated WebP output failed to decode')), { once: true });
    });
    const b64 = dataUrl.split(',')[1] ?? '';
    return { w: img.naturalWidth, h: img.naturalHeight, bytes: Math.floor((b64.length * 3) / 4) };
  }, src);
}

async function expectAnimatedWebp(page, expectedWidth: number) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/webp/);
  const bytes = bytesFromDataUrl(src!);
  expect(bytes.subarray(0, 4).toString('ascii')).toBe('RIFF');
  expect(bytes.subarray(8, 12).toString('ascii')).toBe('WEBP');
  expect(bytes.includes(Buffer.from('ANIM', 'ascii'))).toBeTruthy();
  const info = await imageInfo(page, src!);
  expect(info.w).toBe(expectedWidth);
  expect(info.h).toBe(expectedWidth);
  expect(info.bytes).toBeGreaterThan(100);
}

async function buildArgv(
  page,
  start: number,
  duration: number,
  fps: number,
  width: number,
  quality: number,
  lossless: string,
  inName: string,
) {
  return await page.evaluate(
    async ({ start, duration, fps, width, quality, lossless, inName }) => {
      const mod = await import('/tools/video-to-animated-webp/gizza_ai_video_to_animated_webp_web.js');
      await mod.default('/tools/video-to-animated-webp/gizza_ai_video_to_animated_webp_web_bg.wasm');
      return mod.build_argv(start, duration, fps, width, quality, lossless, inName);
    },
    { start, duration, fps, width, quality, lossless, inName },
  );
}

test('video-to-animated-webp page creates a resized lossy animated WebP', async ({ page }) => {
  await page.goto('/tools/video-to-animated-webp/');
  await page.waitForSelector('#in-file');

  await page.fill('#in-start', '0');
  await page.fill('#in-duration', '0.5');
  await page.fill('#in-fps', '8');
  await page.fill('#in-width', '64');
  await page.fill('#in-quality', '75');
  await page.setInputFiles('#in-file', fixture);

  await expectAnimatedWebp(page, 64);
});

test('video-to-animated-webp deep link preserves a lossless non-default checkbox run', async ({ page }) => {
  await page.goto('/tools/video-to-animated-webp/?start=0&duration=0.5&fps=15&width=32&quality=0&lossless=true');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-duration')).toHaveValue('0.5');
  await expect(page.locator('#in-fps')).toHaveValue('15');
  await expect(page.locator('#in-width')).toHaveValue('32');
  await expect(page.locator('#in-quality')).toHaveValue('0');
  await expect(page.locator('#in-lossless')).toBeChecked();

  await page.setInputFiles('#in-file', fixture);
  await expectAnimatedWebp(page, 32);
});

test('video-to-animated-webp wasm argv covers advertised values and validation', async ({ page }) => {
  await page.goto('/tools/video-to-animated-webp/');
  await page.waitForSelector('#in-file');

  const lossy = await buildArgv(page, 0, 1, 60, 96, 100, 'false', 'in.mp4');
  expect(lossy.out_name).toBe('out.webp');
  expect(lossy.argv).toContain('libwebp');
  expect(lossy.argv[lossy.argv.indexOf('-vf') + 1]).toBe('fps=60,scale=96:-2:flags=lanczos');
  expect(lossy.argv[lossy.argv.indexOf('-quality') + 1]).toBe('100');
  expect(lossy.argv[lossy.argv.indexOf('-loop') + 1]).toBe('0');

  const lossless = await buildArgv(page, 0, 0.5, 12, 64, 80, 'true', 'clip.webm');
  expect(lossless.argv[lossless.argv.indexOf('-lossless') + 1]).toBe('1');
  expect(lossless.argv).not.toContain('-quality');

  await expect(buildArgv(page, 0, 1, 61, 64, 80, 'false', 'in.mp4')).rejects.toThrow(/fps/);
  await expect(buildArgv(page, 0, 1, 12, 4097, 80, 'false', 'in.mp4')).rejects.toThrow(/width/);
});

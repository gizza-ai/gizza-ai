import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/redblue-64.mp4');

async function buildArgv(page, method: string, intensity: number, tint: string, contrast: number, quality: string, keepAudio: string, inName: string) {
  return await page.evaluate(async ({ method, intensity, tint, contrast, quality, keepAudio, inName }) => {
    const mod = await import('/tools/video-grayscale/gizza_ai_video_grayscale_web.js');
    await mod.default('/tools/video-grayscale/gizza_ai_video_grayscale_web_bg.wasm');
    return mod.build_argv(method, intensity, tint, contrast, quality, keepAudio, inName);
  }, { method, intensity, tint, contrast, quality, keepAudio, inName });
}

async function decodeVideo(page) {
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
      v.addEventListener('error', () => reject(new Error('video-grayscale output failed to decode')), { once: true });
    });
    const canvas = document.createElement('canvas');
    canvas.width = v.videoWidth;
    canvas.height = v.videoHeight;
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(v, 0, 0);
    const p = ctx.getImageData(Math.floor(v.videoWidth / 4), Math.floor(v.videoHeight / 2), 1, 1).data;
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration, pixel: [p[0], p[1], p[2]] };
  }, src!);
}

test('video-grayscale wasm build_argv creates the exact default grayscale plan', async ({ page }) => {
  await page.goto('/tools/video-grayscale/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, '', 100, '', 0, '', 'true', 'in.mp4');
  expect(plan.out_name).toBe('out.mp4');
  expect(plan.argv).toEqual([
    '-i', 'in.mp4',
    '-vf', 'colorchannelmixer=rr=0.2126:rg=0.7152:rb=0.0722:gr=0.2126:gg=0.7152:gb=0.0722:br=0.2126:bg=0.7152:bb=0.0722',
    '-c:v', 'libx264',
    '-preset', 'medium',
    '-crf', '23',
    '-c:a', 'copy',
    '-movflags', '+faststart',
    'out.mp4',
  ]);
});

test('video-grayscale wasm build_argv covers advertised values and boundaries', async ({ page }) => {
  await page.goto('/tools/video-grayscale/');
  await page.waitForSelector('#in-file');

  const highContrast = await buildArgv(page, 'bt601', 100, 'sepia', 2, 'best', 'false', 'clip.mov');
  expect(highContrast.out_name).toBe('out.mov');
  expect(highContrast.argv[highContrast.argv.indexOf('-vf') + 1]).toContain('eq=contrast=2');
  expect(highContrast.argv).toEqual(expect.arrayContaining(['-preset', 'slow', '-crf', '20', '-an']));

  const faded = await buildArgv(page, 'average', 50, 'warm', 0.5, 'fast', 'true', 'clip.webm');
  expect(faded.out_name).toBe('out.mp4');
  expect(faded.argv[2]).toBe('-vf');
  expect(faded.argv[3]).toContain('colorchannelmixer=rr=0.6766');
  expect(faded.argv[3]).toContain('eq=contrast=0.5');
  expect(faded.argv).toEqual(expect.arrayContaining(['-preset', 'veryfast', '-crf', '28', '-c:a', 'aac']));

  for (const method of ['red', 'green', 'blue']) {
    const plan = await buildArgv(page, method, 100, 'none', 1, 'balanced', 'true', 'in.mp4');
    expect(plan.argv[3]).toContain('colorchannelmixer=');
  }

  for (const tint of ['none', 'cool', 'cyanotype']) {
    const plan = await buildArgv(page, 'bt709', 100, tint, 1, 'balanced', 'true', 'in.mp4');
    expect(plan.argv[3]).toContain('colorchannelmixer=');
  }
});

test('video-grayscale page renders a playable grayscale video', async ({ page }) => {
  await page.goto('/tools/video-grayscale/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', fixture);
  const frame = await decodeVideo(page);
  expect(frame.w).toBe(64);
  expect(frame.h).toBe(64);
  expect(frame.duration).toBeGreaterThan(0);
  const [r, g, b] = frame.pixel;
  expect(Math.abs(r - g)).toBeLessThanOrEqual(3);
  expect(Math.abs(g - b)).toBeLessThanOrEqual(3);
});

test('video-grayscale deep-link prefills controls and runs a silent sepia output', async ({ page }) => {
  await page.goto('/tools/video-grayscale/?method=bt601&intensity=80&tint=sepia&contrast=1.45&quality=fast&keep_audio=false');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-method')).toHaveValue('bt601');
  await expect(page.locator('#in-intensity')).toHaveValue('80');
  await expect(page.locator('#in-tint')).toHaveValue('sepia');
  await expect(page.locator('#in-contrast')).toHaveValue('1.45');
  await expect(page.locator('#in-quality')).toHaveValue('fast');
  await expect(page.locator('#in-keep_audio')).not.toBeChecked();
  await page.setInputFiles('#in-file', fixture);
  const frame = await decodeVideo(page);
  expect(frame.w).toBe(64);
  expect(frame.h).toBe(64);
});

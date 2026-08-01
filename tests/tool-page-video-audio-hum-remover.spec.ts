import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

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
      v.addEventListener('error', () => reject(new Error('video-audio-hum-remover output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

async function buildArgv(page, frequency, harmonics, q, inName) {
  return page.evaluate(
    async ({ frequency, harmonics, q, inName }) => {
      const mod = await import('/tools/video-audio-hum-remover/gizza_ai_video_audio_hum_remover_web.js');
      await mod.default('/tools/video-audio-hum-remover/gizza_ai_video_audio_hum_remover_web_bg.wasm');
      return mod.build_argv(frequency, harmonics, q, inName);
    },
    { frequency, harmonics, q, inName },
  );
}

test('video-audio-hum-remover page removes default 50 Hz hum path and keeps video playable', async ({ page }) => {
  await page.goto('/tools/video-audio-hum-remover/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-harmonics', '4');
  await page.fill('#in-q', '10');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-audio-hum-remover honors deep-linked 60 Hz parameters', async ({ page }) => {
  await page.goto('/tools/video-audio-hum-remover/?frequency=60&harmonics=0&q=40');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-frequency')).toHaveValue('60');
  await expect(page.locator('#in-harmonics')).toHaveValue('0');
  await expect(page.locator('#in-q')).toHaveValue('40');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableVideoDataUrl(page);
});

test('video-audio-hum-remover wasm build_argv exposes exact notch chains and output names', async ({ page }) => {
  await page.goto('/tools/video-audio-hum-remover/');
  await page.waitForSelector('#in-file');

  await expect(buildArgv(page, '50', 2, 10, 'clip.mp4')).resolves.toEqual({
    argv: [
      '-i',
      'clip.mp4',
      '-c:v',
      'copy',
      '-af',
      'bandreject=f=50:width_type=q:w=10,bandreject=f=100:width_type=q:w=10,bandreject=f=150:width_type=q:w=10',
      '-c:a',
      'aac',
      'out.mp4',
    ],
    out_name: 'out.mp4',
  });

  await expect(buildArgv(page, '60', 0, 40, 'clip.webm')).resolves.toEqual({
    argv: [
      '-i',
      'clip.webm',
      '-c:v',
      'copy',
      '-af',
      'bandreject=f=60:width_type=q:w=40',
      '-c:a',
      'libopus',
      'out.webm',
    ],
    out_name: 'out.webm',
  });
});

import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

const MP4 = path.resolve(__dirname, 'fixtures/stereo-left-only-128x128.mp4');

async function outputSrc(page: Page): Promise<string> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toBeTruthy();
  return src!;
}

async function decodeVideo(
  page: Page,
  src: string,
): Promise<{ w: number; h: number; duration: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener(
        'error',
        () => reject(new Error('video-reverse output failed to decode')),
        { once: true },
      );
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src);
}

async function plan(
  page: any,
  mode = 'reverse',
  audio = 'reverse',
  quality = 'balanced',
  inName = 'clip.mp4',
) {
  return await page.evaluate(async (args) => {
    const mod = await import('/tools/video-reverse/gizza_ai_video_reverse_web.js');
    await mod.default('/tools/video-reverse/gizza_ai_video_reverse_web_bg.wasm');
    return mod.build_argv(args.mode, args.audio, args.quality, args.inName);
  }, { mode, audio, quality, inName });
}

test('video-reverse page exposes presets and CLI copy', async ({ page }) => {
  await page.goto('/tools/video-reverse/');
  await expect(page.locator('#in-file')).toBeVisible({ timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('reverse');
  await expect(page.locator('#in-audio')).toHaveValue('reverse');
  await expect(page.locator('#in-quality')).toHaveValue('balanced');
  await page.selectOption('#in-mode', 'forward-reverse');
  await page.selectOption('#in-audio', 'mute');
  await page.selectOption('#in-quality', 'small');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool video-reverse');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});

test('video-reverse deep-link prefills boomerang and muted audio', async ({ page }) => {
  await page.goto('/tools/video-reverse/?mode=forward-reverse&audio=mute&quality=high');
  await expect(page.locator('#in-mode')).toHaveValue('forward-reverse', { timeout: 15_000 });
  await expect(page.locator('#in-audio')).toHaveValue('mute');
  await expect(page.locator('#in-quality')).toHaveValue('high');
});

test('video-reverse page renders a real muted MP4 output', async ({ page }) => {
  await page.goto('/tools/video-reverse/?mode=reverse&audio=mute&quality=small');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', MP4);

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:video\/mp4/);
  const decoded = await decodeVideo(page, src);
  expect(decoded.w).toBe(128);
  expect(decoded.h).toBe(128);
  expect(decoded.duration).toBeGreaterThan(1.5);
});

test('video-reverse wasm argv covers mode, audio and quality matrix', async ({ page }) => {
  await page.goto('/tools/video-reverse/');
  await page.waitForSelector('#in-file');

  const reverse = await plan(page, 'reverse', 'reverse', 'balanced', 'clip.mov');
  expect(reverse.out_name).toBe('out.mp4');
  expect(reverse.argv).toContain('-filter_complex');
  expect(reverse.argv.join(' ')).toContain('[0:v]reverse[v]');
  expect(reverse.argv.join(' ')).toContain('[0:a]areverse[a]');
  expect(reverse.argv).toContain('23');

  const keep = await plan(page, 'reverse', 'keep', 'small');
  expect(keep.argv.join(' ')).toContain('[0:a]anull[a]');
  expect(keep.argv.join(' ')).not.toContain('areverse[a]');
  expect(keep.argv).toContain('28');

  const mute = await plan(page, 'forward-reverse', 'mute', 'high');
  expect(mute.argv.join(' ')).toContain('concat=n=2:v=1:a=0[v]');
  expect(mute.argv).toContain('-an');
  expect(mute.argv.join(' ')).not.toContain('[a]');
  expect(mute.argv).toContain('18');

  const buildUp = await plan(page, 'reverse-forward', 'reverse', 'balanced');
  expect(buildUp.argv.join(' ')).toContain('[vr][vf]concat=n=2:v=1:a=0[v]');

  await expect(plan(page, 'sideways')).rejects.toThrow(/mode/);
});

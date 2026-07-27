import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/two-audio-128.mp4');

async function buildArgv(page, track: number, keepSubtitles: string, inName: string) {
  return await page.evaluate(async ({ track, keepSubtitles, inName }) => {
    const mod = await import('/tools/video-audio-track-selector/gizza_ai_video_audio_track_selector_web.js');
    await mod.default('/tools/video-audio-track-selector/gizza_ai_video_audio_track_selector_web_bg.wasm');
    return mod.build_argv(track, keepSubtitles, inName);
  }, { track, keepSubtitles, inName });
}

async function decodeVideo(page, src: string) {
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.preload = 'metadata';
    await new Promise((resolve, reject) => {
      v.addEventListener('loadedmetadata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video-audio-track-selector output failed to decode')), { once: true });
      v.src = dataUrl;
    });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
}

async function expectPlayableSingleTrackMp4(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(1.5);
  expect(meta.d).toBeLessThan(2.6);
}

test('video-audio-track-selector wasm build_argv maps exactly one chosen audio track', async ({ page }) => {
  await page.goto('/tools/video-audio-track-selector/');
  await page.waitForSelector('#in-file');

  const second = await buildArgv(page, 1, '', 'in.mp4');
  expect(second.out_name).toBe('out.mp4');
  expect(second.argv).toEqual(['-i', 'in.mp4', '-map', '0:v', '-map', '0:a:1', '-c', 'copy', '-disposition:a:0', 'default', 'out.mp4']);

  const withSubs = await buildArgv(page, 0, 'true', 'clip.mkv');
  expect(withSubs.out_name).toBe('out.mkv');
  expect(withSubs.argv).toContain('0:s?');

  await expect(buildArgv(page, -1, '', 'in.mp4')).rejects.toThrow(/whole number/);
});

test('video-audio-track-selector page keeps the second audio track from a two-audio mp4', async ({ page }) => {
  await page.goto('/tools/video-audio-track-selector/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-track', '1');
  await page.setInputFiles('#in-file', fixture);
  await expectPlayableSingleTrackMp4(page);
});

test('video-audio-track-selector deep-link prefills track and subtitle checkbox', async ({ page }) => {
  await page.goto('/tools/video-audio-track-selector/?track=1&keep_subtitles=true');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-track')).toHaveValue('1');
  await expect(page.locator('#in-keep_subtitles')).toBeChecked();
});

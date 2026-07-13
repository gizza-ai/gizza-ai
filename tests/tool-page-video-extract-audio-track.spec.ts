import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4');

async function buildArgv(page, container: string, track: number, inName: string) {
  return await page.evaluate(async ({ container, track, inName }) => {
    const mod = await import('/tools/video-extract-audio-track/gizza_ai_video_extract_audio_track_web.js');
    await mod.default('/tools/video-extract-audio-track/gizza_ai_video_extract_audio_track_web_bg.wasm');
    return mod.build_argv(container, track, inName);
  }, { container, track, inName });
}

async function decodeAudio(page, src: string) {
  return await page.evaluate(async (dataUrl) => {
    const a = document.createElement('audio');
    a.preload = 'metadata';
    await new Promise((resolve, reject) => {
      a.addEventListener('loadedmetadata', resolve, { once: true });
      a.addEventListener('error', () => reject(new Error('video-extract-audio-track output failed to decode')), { once: true });
      a.src = dataUrl;
    });
    return { duration: a.duration };
  }, src);
}

async function setContainer(page, value: string) {
  const el = page.locator('#in-container');
  const tag = await el.evaluate((node) => node.tagName.toLowerCase());
  if (tag === 'select') {
    await el.selectOption(value);
  } else {
    await el.fill(value);
  }
}

test('video-extract-audio-track wasm build_argv builds the exact lossless demux plan', async ({ page }) => {
  await page.goto('/tools/video-extract-audio-track/');
  await page.waitForSelector('#in-file');

  const mka = await buildArgv(page, 'mka', 0, 'in.mp4');
  expect(mka.out_name).toBe('out.mka');
  expect(mka.argv).toEqual(['-i', 'in.mp4', '-vn', '-map', '0:a:0', '-c:a', 'copy', 'out.mka']);

  const m4a = await buildArgv(page, 'm4a', 0, 'clip.mov');
  expect(m4a.out_name).toBe('out.m4a');
  expect(m4a.argv[m4a.argv.indexOf('-c:a') + 1]).toBe('copy');

  const oggTrack = await buildArgv(page, 'ogg', 1, 'clip.webm');
  expect(oggTrack.out_name).toBe('out.ogg');
  expect(oggTrack.argv).toContain('0:a:1');

  await expect(buildArgv(page, 'mp3', 0, 'in.mp4')).rejects.toThrow(/container/);
});

test('video-extract-audio-track page extracts AAC audio to playable M4A', async ({ page }) => {
  await page.goto('/tools/video-extract-audio-track/');
  await page.waitForSelector('#in-file');
  await setContainer(page, 'm4a');
  await page.fill('#in-track', '0');
  await page.setInputFiles('#in-file', fixture);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/(mp4|x-m4a|mpeg|)/);

  const meta = await decodeAudio(page, src!);
  expect(meta.duration).toBeGreaterThan(0);
});

test('video-extract-audio-track honors container and track deep links', async ({ page }) => {
  await page.goto('/tools/video-extract-audio-track/?container=ogg&track=1');
  await page.waitForSelector('#in-file');
  const container = page.locator('#in-container');
  const tag = await container.evaluate((node) => node.tagName.toLowerCase());
  if (tag === 'select') {
    await expect(container).toHaveValue('ogg');
  } else {
    await expect(container).toHaveValue('ogg');
  }
  await expect(page.locator('#in-track')).toHaveValue('1');
});

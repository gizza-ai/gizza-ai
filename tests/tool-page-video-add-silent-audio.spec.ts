import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-add-silent-audio/ page adds a generated silent
// audio stream to a video, stream-copying the picture in-browser via
// ffmpeg-wasm. These tests assert both the exact wasm-built argv and real media
// output decode for the committed no-audio video fixture.

const noAudioFixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

async function buildArgv(page, channels: string, sampleRate: string, bitrate: string, existingAudio: string, inName: string) {
  return await page.evaluate(async ({ channels, sampleRate, bitrate, existingAudio, inName }) => {
    const mod = await import('/tools/video-add-silent-audio/gizza_ai_video_add_silent_audio_web.js');
    await mod.default('/tools/video-add-silent-audio/gizza_ai_video_add_silent_audio_web_bg.wasm');
    return mod.build_argv(channels, sampleRate, bitrate, existingAudio, inName);
  }, { channels, sampleRate, bitrate, existingAudio, inName });
}

async function decodeVideo(page, src: string) {
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.preload = 'metadata';
    await new Promise((resolve, reject) => {
      v.addEventListener('loadedmetadata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video-add-silent-audio output failed to decode')), { once: true });
      v.src = dataUrl;
    });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
}

async function expectPlayableMp4(page) {
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

test('video-add-silent-audio wasm build_argv creates the exact default silent-track plan', async ({ page }) => {
  await page.goto('/tools/video-add-silent-audio/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, '', '', '', '', 'in.mp4');
  expect(plan.out_name).toBe('out.mp4');
  expect(plan.argv).toEqual([
    '-i', 'in.mp4',
    '-filter_complex', 'anullsrc=channel_layout=stereo:sample_rate=48000[silence]',
    '-map', '0:v',
    '-map', '[silence]',
    '-c:v', 'copy',
    '-c:a', 'aac',
    '-b:a', '128k',
    '-shortest',
    'out.mp4',
  ]);
});

test('video-add-silent-audio wasm build_argv covers advertised enum choices', async ({ page }) => {
  await page.goto('/tools/video-add-silent-audio/');
  await page.waitForSelector('#in-file');

  const mono = await buildArgv(page, 'mono', '22050', '32', 'replace', 'in.mov');
  expect(mono.out_name).toBe('out.mov');
  expect(mono.argv).toContain('anullsrc=channel_layout=mono:sample_rate=22050[silence]');
  expect(mono.argv).toContain('32k');

  const keep = await buildArgv(page, 'stereo', '44100', '192', 'keep', 'in.mp4');
  expect(keep.argv).toEqual(expect.arrayContaining(['0:a?', '192k']));
  expect(keep.argv.filter((arg: string) => arg === '-map')).toHaveLength(3);

  const webm = await buildArgv(page, 'stereo', '22050', '64', 'replace', 'in.webm');
  expect(webm.out_name).toBe('out.webm');
  expect(webm.argv).toEqual(expect.arrayContaining(['libopus', '64k']));
  expect(webm.argv).toContain('anullsrc=channel_layout=stereo:sample_rate=48000[silence]');
});

test('video-add-silent-audio page adds a silent audio track and preserves playable video', async ({ page }) => {
  await page.goto('/tools/video-add-silent-audio/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-bitrate', '64');
  await page.setInputFiles('#in-file', noAudioFixture);
  await expectPlayableMp4(page);
});

test('video-add-silent-audio deep-link prefills selects and runs on upload', async ({ page }) => {
  await page.goto('/tools/video-add-silent-audio/?channels=mono&sample_rate=22050&bitrate=32&existing_audio=replace');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-channels')).toHaveValue('mono');
  await expect(page.locator('#in-sample_rate')).toHaveValue('22050');
  await expect(page.locator('#in-bitrate')).toHaveValue('32');
  await expect(page.locator('#in-existing_audio')).toHaveValue('replace');

  await page.setInputFiles('#in-file', noAudioFixture);
  await expectPlayableMp4(page);
});

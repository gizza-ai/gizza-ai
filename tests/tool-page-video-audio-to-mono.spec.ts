import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/video-audio-to-mono/ page downmixes an uploaded video's
// audio to one mono channel in-browser via ffmpeg (@ffmpeg/core from jsDelivr —
// needs network), stream-copying the picture.
//
// The mp4 fixture is deliberately ONE-SIDED: 128x128 h264 video plus a 2s
// STEREO aac track whose left channel is a 440 Hz sine and whose right channel
// is digital silence. That asymmetry is what makes the channel control
// testable — `left` must come back loud, `right` must come back silent, and
// `mix` must land in between. Every assertion below is a RATIO or a
// same-fixture comparison, never an absolute dBFS window, because lavfi's
// `sine` is ~1/8 scale rather than full scale.
const MP4 = path.resolve(__dirname, 'fixtures/stereo-left-only-128x128.mp4');
// Secondary input format: vp8 + vorbis, so the output goes webm/libopus and
// exercises the sample-rate snap (libopus rejects 44100 outright).
const WEBM = path.resolve(__dirname, 'fixtures/clip-1s.webm');

type Decoded = { channels: number; rms: number; duration: number };

async function outputSrc(page: Page): Promise<string> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toBeTruthy();
  return src!;
}

/// Decode the produced media's AUDIO with WebAudio and report channel count,
/// full-clip RMS and duration. decodeAudioData reads the audio track out of an
/// mp4/webm container directly.
async function decodeAudio(page: Page, src: string): Promise<Decoded> {
  return page.evaluate(async (dataUrl: string) => {
    const buf = await (await fetch(dataUrl)).arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    const data = decoded.getChannelData(0);
    let sum = 0;
    for (let i = 0; i < data.length; i++) sum += data[i] * data[i];
    return {
      channels: decoded.numberOfChannels,
      rms: Math.sqrt(sum / Math.max(1, data.length)),
      duration: decoded.duration,
    };
  }, src);
}

/// Decode the produced media's PICTURE and report its dimensions, proving the
/// stream copy left the video track intact.
async function decodeVideo(
  page: Page,
  src: string
): Promise<{ w: number; h: number; duration: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener(
        'error',
        () => reject(new Error('video-audio-to-mono output failed to decode')),
        { once: true }
      );
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src);
}

test('video-audio-to-mono page downmixes to one channel and keeps the picture', async ({
  page,
}) => {
  await page.goto('/tools/video-audio-to-mono/');
  await page.waitForSelector('#in-file');
  // Defaults: channel=mix, bitrate=128, sample_rate=keep.
  await expect(page.locator('#in-channel')).toHaveValue('mix');
  await expect(page.locator('#in-sample_rate')).toHaveValue('keep');
  await page.setInputFiles('#in-file', MP4);

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:video\/mp4/);
  const frame = await decodeVideo(page, src);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(1.5);

  const audio = await decodeAudio(page, src);
  expect(audio.channels).toBe(1); // the whole point of the tool
  expect(audio.duration).toBeGreaterThan(1.5);
  expect(audio.rms).toBeGreaterThan(0.001); // mixing a live L with a silent R still has signal
});

test('video-audio-to-mono page honors query params and picks the left channel', async ({
  page,
}) => {
  await page.goto(
    '/tools/video-audio-to-mono/?channel=left&bitrate=64&sample_rate=16000'
  );
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-channel')).toHaveValue('left');
  await expect(page.locator('#in-bitrate')).toHaveValue('64');
  await expect(page.locator('#in-sample_rate')).toHaveValue('16000');
  await page.setInputFiles('#in-file', MP4);

  const audio = await decodeAudio(page, await outputSrc(page));
  expect(audio.channels).toBe(1);
  // The fixture's loud side — must survive at full level.
  expect(audio.rms).toBeGreaterThan(0.01);
});

test('video-audio-to-mono page picking the dead right channel yields near-silence', async ({
  page,
}) => {
  // Same fixture, opposite choice: this is the assertion that proves the
  // channel control actually routes rather than just re-encoding.
  await page.goto('/tools/video-audio-to-mono/?channel=right');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', MP4);

  const audio = await decodeAudio(page, await outputSrc(page));
  expect(audio.channels).toBe(1);
  expect(audio.rms).toBeLessThan(0.001); // the fixture's right channel is digital silence
});

test('video-audio-to-mono page difference mode keeps the uncancelled side', async ({
  page,
}) => {
  // L - R with a silent R is L, so the difference output must be loud, not
  // silent — distinguishing it from the `right` case above.
  await page.goto('/tools/video-audio-to-mono/?channel=difference');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', MP4);

  const audio = await decodeAudio(page, await outputSrc(page));
  expect(audio.channels).toBe(1);
  expect(audio.rms).toBeGreaterThan(0.01);
});

test('video-audio-to-mono page handles webm at the min bitrate with a snapped rate', async ({
  page,
}) => {
  // Secondary input format + both edges of the advertised matrix: the 16 kbps
  // floor, and a 44100 Hz request that libopus cannot open (it must be snapped
  // to 48000 by core, or ffmpeg hard-fails with "sample rate not supported").
  await page.goto(
    '/tools/video-audio-to-mono/?channel=mix&bitrate=16&sample_rate=44100'
  );
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', WEBM);

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:video\/webm/);
  const audio = await decodeAudio(page, src);
  expect(audio.channels).toBe(1);
  expect(audio.duration).toBeGreaterThan(0);
});

test('video-audio-to-mono page accepts the top of the bitrate range', async ({
  page,
}) => {
  await page.goto('/tools/video-audio-to-mono/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-bitrate', '320');
  await page.setInputFiles('#in-file', MP4);

  const audio = await decodeAudio(page, await outputSrc(page));
  expect(audio.channels).toBe(1);
});

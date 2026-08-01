import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-resampler/ page resamples an uploaded audio file
// in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network). Output is
// audio, so the media element is an <audio> with a data:audio/ src. The first
// test parses the produced WAV header (offset 24 = sample rate, u32 LE) to prove
// the output is EXACTLY the requested rate, and decodes it with WebAudio to prove
// the full source duration survived. The deep-link test proves prefilling + a
// FLAC target (browser ffmpeg ships libFLAC).

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3'); // 3.03s tone

async function decodeDuration(page: Page, src: string): Promise<number> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return decoded.duration;
  }, src);
}

// Read the sample rate straight from the WAV header (bytes 24-27, little-endian).
async function wavSampleRate(page: Page, src: string): Promise<number> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    return new DataView(buf).getUint32(24, true);
  }, src);
}

test('audio-resampler page resamples to exactly 16000 Hz WAV', async ({ page }) => {
  await page.goto('/tools/audio-resampler/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-rate', '16000');
  await page.selectOption('#in-format', 'wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  expect(await wavSampleRate(page, src!)).toBe(16000);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.2);
});

test('audio-resampler deep link prefills rate=48000&format=flac', async ({ page }) => {
  await page.goto('/tools/audio-resampler/?rate=48000&format=flac');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-rate')).toHaveValue('48000', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('flac');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/(flac|x-flac)/);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.2);
});

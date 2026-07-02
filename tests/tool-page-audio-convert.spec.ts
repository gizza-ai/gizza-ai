import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-convert/ page re-encodes an uploaded audio file
// in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network). Output is
// audio, so the media element is an <audio> with a data:audio/ src. Beyond the
// mime prefix, each test decodes the result with WebAudio and asserts the full
// source duration survived the conversion (media-correctness rule). The ogg
// case also proves the browser ffmpeg build ships libvorbis.

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

test('audio-convert page converts an mp3 to lossless wav', async ({ page }) => {
  await page.goto('/tools/audio-convert/');
  await page.waitForSelector('#in-audio');
  await page.selectOption('#in-format', 'wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.2);
});

test('audio-convert deep link prefills format=ogg and converts via libvorbis', async ({ page }) => {
  await page.goto('/tools/audio-convert/?format=ogg&bitrate=96');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-format')).toHaveValue('ogg', { timeout: 15_000 });
  await expect(page.locator('#in-bitrate')).toHaveValue('96');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.2);
});

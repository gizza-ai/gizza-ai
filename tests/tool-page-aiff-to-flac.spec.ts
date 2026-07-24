import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/aiff-to-flac/ page re-encodes an uploaded audio file
// in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network). Output is
// FLAC audio, so the media element is an <audio> with a data:audio/flac src.
// Decode the result with WebAudio to prove real media output, not just argv text.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.wav');

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

test('aiff-to-flac page converts audio to lossless flac at default compression', async ({ page }) => {
  await page.goto('/tools/aiff-to-flac/');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-compression_level')).toHaveAttribute('placeholder', '5', { timeout: 15_000 });
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.2);
});

test('aiff-to-flac deep link prefills maximum compression and still converts', async ({ page }) => {
  await page.goto('/tools/aiff-to-flac/?compression_level=12');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-compression_level')).toHaveValue('12', { timeout: 15_000 });
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.2);
});

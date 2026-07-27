import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-channel/ page re-routes an uploaded file's
// channels in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network).
// The fixture is stereo with DIFFERENT content per side: 440 Hz on the left,
// 880 Hz on the right. Decoding the output proves routing spectrally. For the
// default `swap`, the left channel of the output must carry the ORIGINAL right
// tone (880 Hz ≈ 1760 zero-crossings/s), not the 440 Hz it started with — proof
// the sides actually moved. For `mono` the output must collapse to one channel.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-stereo-3s.mp3');

async function decodeStats(
  page: Page,
  src: string,
): Promise<{ channels: number; duration: number; crossingsPerSec: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    const data = decoded.getChannelData(0); // left channel
    let crossings = 0;
    for (let i = 1; i < data.length; i++) {
      if ((data[i - 1] < 0 && data[i] >= 0) || (data[i - 1] >= 0 && data[i] < 0)) crossings++;
    }
    return {
      channels: decoded.numberOfChannels,
      duration: decoded.duration,
      crossingsPerSec: crossings / decoded.duration,
    };
  }, src);
}

test('audio-channel page swaps left and right channels', async ({ page }) => {
  await page.goto('/tools/audio-channel/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { channels, duration, crossingsPerSec } = await decodeStats(page, src!);
  expect(channels).toBe(2); // stays stereo
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
  expect(crossingsPerSec).toBeGreaterThan(1500); // left now holds the 880 Hz right tone (~1760/s)
});

test('audio-channel deep link downmixes to mono as wav', async ({ page }) => {
  await page.goto('/tools/audio-channel/?operation=mono&format=wav');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-operation')).toHaveValue('mono', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { channels, duration } = await decodeStats(page, src!);
  expect(channels).toBe(1);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
});

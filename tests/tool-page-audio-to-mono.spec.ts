import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-to-mono/ page downmixes an uploaded file
// in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network). The
// fixture is stereo with DIFFERENT content per side: 440 Hz on the left, 880 Hz
// on the right. Decoding the output proves correctness two ways: the buffer
// must be mono (numberOfChannels === 1), and for channel=left the zero-crossing
// rate must sit near a pure 440 Hz tone's (~880 crossings/s) — evidence the
// right side really was discarded, not blended in (a mix would cross far more
// often).

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
    const data = decoded.getChannelData(0);
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

test('audio-to-mono page mixes a stereo file down to one channel', async ({ page }) => {
  await page.goto('/tools/audio-to-mono/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { channels, duration } = await decodeStats(page, src!);
  expect(channels).toBe(1);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
});

test('audio-to-mono deep link keeps only the 440 Hz left channel', async ({ page }) => {
  await page.goto('/tools/audio-to-mono/?channel=left&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-channel')).toHaveValue('left', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { channels, crossingsPerSec } = await decodeStats(page, src!);
  expect(channels).toBe(1);
  expect(crossingsPerSec).toBeGreaterThan(750); // pure 440 Hz ≈ 880 crossings/s
  expect(crossingsPerSec).toBeLessThan(1000); // right channel (880 Hz → ~1760/s) absent
});

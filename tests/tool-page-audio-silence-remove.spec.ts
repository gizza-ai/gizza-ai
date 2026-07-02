import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-silence-remove/ page strips silent gaps from an
// uploaded recording in-browser via ffmpeg silenceremove (@ffmpeg/core from
// jsDelivr — needs network). The fixture is 0.7s tone + 1.5s silence + 0.7s
// tone (~2.95s total). With the defaults (-30 dB, 0.5s min gap, 0.25s of each
// gap kept) the output must collapse to ~0.7 + 0.25 + 0.7 ≈ 1.65s — decoding
// and measuring the duration proves silence was actually removed.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-gaps-3s.mp3');

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

test('audio-silence-remove page collapses a 1.5s gap with the defaults', async ({ page }) => {
  await page.goto('/tools/audio-silence-remove/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(1.2); // both tones + kept 0.25s survive
  expect(duration).toBeLessThan(2.2); // the 1.5s gap is gone
});

test('audio-silence-remove deep link raises the min gap so nothing is trimmed', async ({ page }) => {
  // min_silence=2 > the 1.5s gap → the recording must come back full length.
  await page.goto('/tools/audio-silence-remove/?min_silence=2&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-min_silence')).toHaveValue('2', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(2.8); // gap shorter than min_silence is kept
  expect(duration).toBeLessThan(3.2);
});

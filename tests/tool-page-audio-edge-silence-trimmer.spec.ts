import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-edge-silence-trimmer/ page trims only the leading
// and trailing silence from an uploaded recording in-browser via ffmpeg
// silenceremove + areverse (@ffmpeg/core from jsDelivr — needs network). The
// fixture is 1.2s silence + 1.2s of 440Hz tone + 1.2s silence (~3.6s total).
// With the defaults (-50 dB, 0.1s pad) the output must collapse to roughly
// 0.1 + 1.2 + 0.1 ≈ 1.4s — decoding and measuring the duration proves the edge
// silence was actually removed while the body survived.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-edges-silence-4s.mp3');

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

test('audio-edge-silence-trimmer page trims the leading and trailing silence', async ({ page }) => {
  await page.goto('/tools/audio-edge-silence-trimmer/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(1.0); // the tone body + edge pads survive
  expect(duration).toBeLessThan(2.4); // ~2.4s of edge silence is gone
});

test('audio-edge-silence-trimmer deep link applies a hard cut and wav output', async ({ page }) => {
  // pad=0 → no edge padding kept, so the output is a touch tighter than the
  // default and comes back as WAV.
  await page.goto('/tools/audio-edge-silence-trimmer/?threshold_db=-45&pad=0&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-threshold_db')).toHaveValue('-45', { timeout: 15_000 });
  await expect(page.locator('#in-pad')).toHaveValue('0');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(1.0); // the tone body survives
  expect(duration).toBeLessThan(2.2); // edge silence trimmed, no pad kept
});

import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-compressor/ page applies dynamic-range compression
// to an uploaded file in-browser via ffmpeg `acompressor` (@ffmpeg/core from
// jsDelivr — needs network). The fixture is a very quiet 3s 440 Hz tone (RMS ≈
// -47.5 dBFS / 0.0042). The default (threshold -20, ratio 4, no make-up) mostly
// leaves a tone this quiet alone, so the default test proves a real audio encode
// with the duration preserved. The deep-link test uses ratio 1 with +12 dB
// make-up gain: a documented, valid clean level boost (ratio 1 is only a no-op
// when make-up is 0). Decoding the wav output and measuring RMS proves the
// control actually took effect: +12 dB should land around 0.016-0.018.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-quiet-3s.mp3');

async function decodeStats(page: Page, src: string): Promise<{ duration: number; rms: number; bytes: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const bytes = buf.byteLength;
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    const data = decoded.getChannelData(0);
    let sum = 0;
    for (let i = 0; i < data.length; i++) sum += data[i] * data[i];
    return { duration: decoded.duration, rms: Math.sqrt(sum / data.length), bytes };
  }, src);
}

test('audio-compressor page runs the default compressor and preserves the audio', async ({ page }) => {
  await page.goto('/tools/audio-compressor/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE); // all controls blank → defaults, mp3
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, bytes } = await decodeStats(page, src!);
  expect(bytes).toBeGreaterThan(2_000); // a real encode, not a stub
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
});

test('audio-compressor deep link prefills controls and applies +12 dB make-up gain (wav)', async ({ page }) => {
  await page.goto('/tools/audio-compressor/?threshold=-24&ratio=1&makeup=12&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-threshold')).toHaveValue('-24', { timeout: 15_000 });
  await expect(page.locator('#in-ratio')).toHaveValue('1');
  await expect(page.locator('#in-makeup')).toHaveValue('12');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, rms } = await decodeStats(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
  expect(rms).toBeGreaterThan(0.014); // input RMS ≈ 0.0042, +12 dB ≈ ×3.98 → ≈ 0.0167
  expect(rms).toBeLessThan(0.02);
});

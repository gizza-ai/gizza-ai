import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

// The generated /tools/audio-eq/ page equalizes an uploaded file in-browser
// via ffmpeg (@ffmpeg/core from jsDelivr — needs network). Assertions are
// RATIO-based (output RMS ÷ input RMS, both decoded via WebAudio) against
// single-tone fixtures parked inside one band, so the shelf gain is measurable:
// a -15 dB bass cut on a 50 Hz tone must scale RMS by ~0.20 (pre-measured
// -13.8 dB — the tone sits near the 100 Hz shelf corner), and a +12 dB treble
// boost on an 8 kHz tone by ~3.9 (pre-measured +11.8 dB). Ratios are immune to
// the lavfi-sine ~1/8-amplitude fixture gotcha.

const FIXTURE_50HZ = path.resolve(__dirname, 'fixtures/tone-50hz-3s.mp3');
const FIXTURE_8KHZ = path.resolve(__dirname, 'fixtures/tone-8000hz-3s.mp3');

async function rmsOfData(page: Page, bytes: Buffer | ArrayBuffer): Promise<number> {
  const b64 = Buffer.isBuffer(bytes) ? bytes.toString('base64') : Buffer.from(bytes).toString('base64');
  return page.evaluate(async (b64data: string) => {
    const bin = atob(b64data);
    const arr = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(arr.buffer);
    await ctx.close();
    const data = decoded.getChannelData(0);
    let sum = 0;
    for (let i = 0; i < data.length; i++) sum += data[i] * data[i];
    return Math.sqrt(sum / data.length);
  }, b64);
}

async function rmsOfDataUrl(page: Page, src: string): Promise<number> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    const data = decoded.getChannelData(0);
    let sum = 0;
    for (let i = 0; i < data.length; i++) sum += data[i] * data[i];
    return Math.sqrt(sum / data.length);
  }, src);
}

test('audio-eq page cuts a 50 Hz tone by -15 dB bass (RMS ×~0.20)', async ({ page }) => {
  await page.goto('/tools/audio-eq/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-bass', '-15');
  const inputRms = await rmsOfData(page, fs.readFileSync(FIXTURE_50HZ));
  await page.setInputFiles('#in-audio', FIXTURE_50HZ);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const outRms = await rmsOfDataUrl(page, src!);
  const ratio = outRms / inputRms;
  expect(ratio).toBeGreaterThan(0.13); // pre-measured -13.8 dB ⇒ ×0.204
  expect(ratio).toBeLessThan(0.3);
});

test('audio-eq deep link boosts an 8 kHz tone by +12 dB treble (RMS ×~3.9)', async ({ page }) => {
  await page.goto('/tools/audio-eq/?treble=12&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-treble')).toHaveValue('12', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('wav');
  const inputRms = await rmsOfData(page, fs.readFileSync(FIXTURE_8KHZ));
  await page.setInputFiles('#in-audio', FIXTURE_8KHZ);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const outRms = await rmsOfDataUrl(page, src!);
  const ratio = outRms / inputRms;
  expect(ratio).toBeGreaterThan(3.0); // pre-measured +11.8 dB ⇒ ×3.9, wav exact
  expect(ratio).toBeLessThan(5.0);
});

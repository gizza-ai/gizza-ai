import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

// The generated /tools/audio-volume-adjust/ page re-gains an uploaded file
// in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network). The
// fixture is a quiet 3s tone (RMS ≈ 0.0042 — note ffmpeg's lavfi sine source
// generates at ~1/8 amplitude, which an earlier draft of this spec learned the
// hard way). Assertions are RATIO-based: the output RMS divided by the
// fixture's decoded RMS must equal the requested gain (+6 dB → ×2.0,
// factor 0.5 → ×0.5), which proves the gain was really applied and is immune
// to fixture amplitude assumptions. The quiet tone stays far below the
// limiter's ceiling, so the gain math is exact.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-quiet-3s.mp3');

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

test('audio-volume-adjust page boosts by the +6 dB default (RMS doubles)', async ({ page }) => {
  await page.goto('/tools/audio-volume-adjust/');
  await page.waitForSelector('#in-audio');
  const inputRms = await rmsOfData(page, fs.readFileSync(FIXTURE));
  await page.setInputFiles('#in-audio', FIXTURE); // amount left blank → +6 dB
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const outRms = await rmsOfDataUrl(page, src!);
  const gain = outRms / inputRms;
  expect(gain).toBeGreaterThan(1.7); // +6 dB = ×1.995 (mp3 re-encode wiggle)
  expect(gain).toBeLessThan(2.3);
});

test('audio-volume-adjust deep link halves the amplitude with factor 0.5', async ({ page }) => {
  await page.goto('/tools/audio-volume-adjust/?amount=0.5&unit=factor&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-amount')).toHaveValue('0.5', { timeout: 15_000 });
  await expect(page.locator('#in-unit')).toHaveValue('factor');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  const inputRms = await rmsOfData(page, fs.readFileSync(FIXTURE));
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const outRms = await rmsOfDataUrl(page, src!);
  const gain = outRms / inputRms;
  expect(gain).toBeGreaterThan(0.42); // factor 0.5, wav output is exact
  expect(gain).toBeLessThan(0.58);
});

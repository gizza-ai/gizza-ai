import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

// The generated /tools/audio-highpass-filter/ page applies ffmpeg's highpass
// filter in-browser (@ffmpeg/core from jsDelivr — needs network). Assertions are
// ratio-based against a 50 Hz sine fixture: a steep 120 Hz high-pass should make
// that low tone much quieter, while keeping the output decodable.

const FIXTURE_50HZ = path.resolve(__dirname, 'fixtures/tone-50hz-3s.mp3');

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

async function runAndMeasure(page: Page, fixture: string) {
  await page.setInputFiles('#in-file', fixture);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  return { src: src!, rms: await rmsOfDataUrl(page, src!) };
}

test('audio-highpass-filter page cuts a 50 Hz rumble tone with a steep 120 Hz filter', async ({
  page,
}) => {
  await page.goto('/tools/audio-highpass-filter/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-cutoff', '120');
  await page.selectOption('#in-rolloff', '48');
  const inputRms = await rmsOfData(page, fs.readFileSync(FIXTURE_50HZ));
  const out = await runAndMeasure(page, FIXTURE_50HZ);
  const ratio = out.rms / inputRms;
  expect(ratio).toBeLessThan(0.25);
});

test('audio-highpass-filter deep link drives cutoff, rolloff, and wav output', async ({ page }) => {
  await page.goto('/tools/audio-highpass-filter/?cutoff=120&rolloff=24&format=wav');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-cutoff')).toHaveValue('120', { timeout: 15_000 });
  await expect(page.locator('#in-rolloff')).toHaveValue('24');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  const inputRms = await rmsOfData(page, fs.readFileSync(FIXTURE_50HZ));
  const out = await runAndMeasure(page, FIXTURE_50HZ);
  expect(out.src).toMatch(/^data:audio\/(wav|x-wav)/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.wav');
  const ratio = out.rms / inputRms;
  expect(ratio).toBeLessThan(0.55);
});

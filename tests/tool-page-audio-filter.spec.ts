import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

// The generated /tools/audio-filter/ page applies one of ffmpeg's four classic
// filters (lowpass/highpass/bandpass/bandreject) in-browser (@ffmpeg/core from
// jsDelivr — needs network). Assertions are ratio-based: a low-pass well below a
// pure high tone should make it much quieter, a high-pass well above a low tone
// likewise, while keeping the output decodable.

const FIXTURE_8KHZ = path.resolve(__dirname, 'fixtures/tone-8000hz-3s.mp3');
const FIXTURE_50HZ = path.resolve(__dirname, 'fixtures/tone-50hz-3s.mp3');

async function rmsOfData(page: Page, bytes: Buffer): Promise<number> {
  const b64 = bytes.toString('base64');
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

test('audio-filter low-pass at 500 Hz strongly attenuates an 8 kHz tone', async ({ page }) => {
  await page.goto('/tools/audio-filter/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-type', 'lowpass');
  await page.fill('#in-frequency', '500');
  const inputRms = await rmsOfData(page, fs.readFileSync(FIXTURE_8KHZ));
  const out = await runAndMeasure(page, FIXTURE_8KHZ);
  expect(out.src).toMatch(/^data:audio\/mpeg/);
  const ratio = out.rms / inputRms;
  expect(ratio).toBeLessThan(0.25);
});

test('audio-filter deep link drives high-pass + wav and cuts a 50 Hz rumble tone', async ({
  page,
}) => {
  await page.goto('/tools/audio-filter/?type=highpass&frequency=500&width=200&format=wav');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-type')).toHaveValue('highpass', { timeout: 15_000 });
  await expect(page.locator('#in-frequency')).toHaveValue('500');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  const inputRms = await rmsOfData(page, fs.readFileSync(FIXTURE_50HZ));
  const out = await runAndMeasure(page, FIXTURE_50HZ);
  expect(out.src).toMatch(/^data:audio\/(wav|x-wav)/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.wav');
  const ratio = out.rms / inputRms;
  expect(ratio).toBeLessThan(0.25);
});

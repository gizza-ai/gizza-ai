import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-8000hz-3s.mp3');

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

async function rmsOfInput(page: Page): Promise<number> {
  const b64 = fs.readFileSync(FIXTURE).toString('base64');
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

async function run(page: Page) {
  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  return { src: src!, stats: await decodeStats(page, src!) };
}

test('de-esser page renders an audio output and attenuates an 8 kHz sibilance tone', async ({ page }) => {
  await page.goto('/tools/de-esser/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-amount', '90');
  await page.fill('#in-band', '70');
  await page.fill('#in-max_reduction', '80');
  await page.selectOption('#in-mode', 'output');
  await page.selectOption('#in-format', 'wav');
  const inputRms = await rmsOfInput(page);
  const out = await run(page);
  expect(out.src).toMatch(/^data:audio\/(wav|x-wav)/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.wav');
  expect(out.stats.duration).toBeGreaterThan(2.9);
  expect(out.stats.duration).toBeLessThan(3.3);
  expect(out.stats.rms).toBeLessThan(inputRms * 0.9);
});

test('de-esser deep link prefills ess audition mode and flac output', async ({ page }) => {
  await page.goto('/tools/de-esser/?amount=60&band=70&max_reduction=50&mode=ess&format=flac');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-amount')).toHaveValue('60', { timeout: 15_000 });
  await expect(page.locator('#in-band')).toHaveValue('70');
  await expect(page.locator('#in-max_reduction')).toHaveValue('50');
  await expect(page.locator('#in-mode')).toHaveValue('ess');
  await expect(page.locator('#in-format')).toHaveValue('flac');
  const out = await run(page);
  expect(out.src).toMatch(/^data:audio\/flac/);
  expect(out.stats.bytes).toBeGreaterThan(1_000);
  expect(out.stats.duration).toBeGreaterThan(2.9);
  expect(out.stats.duration).toBeLessThan(3.3);
});

import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-gaps-3s.mp3');

async function durationOfBytes(page: Page, bytes: Buffer | ArrayBuffer): Promise<number> {
  const b64 = Buffer.isBuffer(bytes) ? bytes.toString('base64') : Buffer.from(bytes).toString('base64');
  return page.evaluate(async (b64data: string) => {
    const bin = atob(b64data);
    const arr = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(arr.buffer.slice(0));
    await ctx.close();
    return decoded.duration;
  }, b64);
}

async function durationOfDataUrl(page: Page, src: string): Promise<number> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return decoded.duration;
  }, src);
}

test('audio-pause-shortener page shortens long gaps to a tighter pacing', async ({ page }) => {
  await page.goto('/tools/audio-pause-shortener/');
  await page.waitForSelector('#in-audio');
  const inputDuration = await durationOfBytes(page, fs.readFileSync(FIXTURE));
  await page.fill('#in-threshold_db', '-35');
  await page.fill('#in-max_pause', '0.5');
  await page.fill('#in-target_pause', '0.1');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const outDuration = await durationOfDataUrl(page, src!);
  expect(outDuration).toBeLessThan(inputDuration - 0.5);
  expect(outDuration).toBeGreaterThan(1.5);
});

test('audio-pause-shortener deep link applies tighter pause settings and wav output', async ({ page }) => {
  await page.goto('/tools/audio-pause-shortener/?threshold_db=-35&max_pause=0.5&target_pause=0.1&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-threshold_db')).toHaveValue('-35', { timeout: 15_000 });
  await expect(page.locator('#in-max_pause')).toHaveValue('0.5');
  await expect(page.locator('#in-target_pause')).toHaveValue('0.1');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  const inputDuration = await durationOfBytes(page, fs.readFileSync(FIXTURE));
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const outDuration = await durationOfDataUrl(page, src!);
  expect(outDuration).toBeLessThan(inputDuration - 0.5);
}
);

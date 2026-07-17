import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3');

async function decodeInfo(page: Page, src: string): Promise<{ duration: number; bytes: number; rms: number }> {
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
    return { duration: decoded.duration, bytes, rms: Math.sqrt(sum / Math.max(1, data.length)) };
  }, src);
}

test('audio-effects-rack applies a hall echo chain and returns playable audio', async ({ page }) => {
  await page.goto('/tools/audio-effects-rack/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-reverb', 'hall');
  await page.fill('#in-echo', '250');
  await page.selectOption('#in-chorus', 'light');
  await page.selectOption('#in-compression', 'medium');
  await page.selectOption('#in-format', 'mp3');
  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, bytes, rms } = await decodeInfo(page, src!);
  expect(bytes).toBeGreaterThan(5_000);
  // Echo + reverb append a tail, so the output should be at least as long as the 3s fixture.
  expect(duration).toBeGreaterThan(3.0);
  expect(duration).toBeLessThan(5.0);
  expect(rms).toBeGreaterThan(0.001);
});

test('audio-effects-rack deep link prefills non-default controls and outputs wav tremolo', async ({ page }) => {
  await page.goto('/tools/audio-effects-rack/?tremolo=6&reverb=room&format=wav&compression=heavy');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-tremolo')).toHaveValue('6', { timeout: 15_000 });
  await expect(page.locator('#in-reverb')).toHaveValue('room');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await expect(page.locator('#in-compression')).toHaveValue('heavy');
  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, bytes } = await decodeInfo(page, src!);
  expect(bytes).toBeGreaterThan(100_000); // WAV output proves the format param took effect.
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(4.5);
});

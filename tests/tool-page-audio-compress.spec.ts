import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

// The generated /tools/audio-compress/ page re-encodes an uploaded file at a
// lower bitrate in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs
// network). The fixture is a 3s tone encoded at 192 kbps (~73 KB) so "the
// output is actually smaller" is provable: the default 96 kbps roughly halves
// it, and the 32 kbps deep-link case shrinks it far more. Each test also
// decodes the result with WebAudio and asserts the full source duration
// survived (media-correctness rule).

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-192k-3s.mp3');

async function decodeInfo(
  page: Page,
  src: string
): Promise<{ duration: number; bytes: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const bytes = buf.byteLength;
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return { duration: decoded.duration, bytes };
  }, src);
}

test('audio-compress page halves a 192 kbps mp3 at the 96 kbps default', async ({ page }) => {
  const inputSize = fs.statSync(FIXTURE).size; // ~73 KB
  await page.goto('/tools/audio-compress/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE); // bitrate blank → 96 kbps mp3
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, bytes } = await decodeInfo(page, src!);
  expect(bytes).toBeGreaterThan(10_000); // a real encode, not a stub
  expect(bytes).toBeLessThan(inputSize * 0.65); // 96k vs 192k ≈ half the size
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.2);
});

test('audio-compress deep link prefills bitrate=32 format=ogg and shrinks far more', async ({ page }) => {
  const inputSize = fs.statSync(FIXTURE).size;
  await page.goto('/tools/audio-compress/?bitrate=32&format=ogg');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-bitrate')).toHaveValue('32', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('ogg');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, bytes } = await decodeInfo(page, src!);
  expect(bytes).toBeGreaterThan(2_000);
  expect(bytes).toBeLessThan(inputSize * 0.35); // 32 kbps ≪ the 192 kbps source
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.2);
});

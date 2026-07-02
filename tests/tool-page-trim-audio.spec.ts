import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/trim-audio/ page trims an uploaded audio file in-browser
// via ffmpeg (@ffmpeg/core from jsDelivr — needs network). Output is audio, so
// the media element is an <audio> with a data:audio/ src. Beyond the mime
// prefix, each test decodes the result with WebAudio and asserts the duration
// actually matches the [start, end] selection (media-correctness rule).

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3');

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

test('trim-audio page keeps a 1s selection of an uploaded mp3', async ({ page }) => {
  await page.goto('/tools/trim-audio/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-start', '0.5');
  await page.fill('#in-end', '1.5');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(0.85); // mp3 encoder pads a little
  expect(duration).toBeLessThan(1.2);
});

test('trim-audio deep link prefills fields and trims to wav', async ({ page }) => {
  await page.goto('/tools/trim-audio/?start=1&end=2&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-start')).toHaveValue('1', { timeout: 15_000 });
  await expect(page.locator('#in-end')).toHaveValue('2');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(0.98); // wav is sample-exact
  expect(duration).toBeLessThan(1.02);
});

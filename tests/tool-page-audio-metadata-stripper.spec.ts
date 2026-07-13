import { test, expect } from './fixtures';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-quiet-3s.mp3');

test('audio-metadata-stripper page outputs playable audio with default cover-art removal', async ({ page }) => {
  await page.goto('/tools/audio-metadata-stripper/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return decoded.duration;
  }, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
});

test('audio-metadata-stripper deep link can keep cover art option', async ({ page }) => {
  await page.goto('/tools/audio-metadata-stripper/?cover_art=keep');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-cover_art')).toHaveValue('keep', { timeout: 15_000 });
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect(await media.getAttribute('src')).toMatch(/^data:audio\//);
});
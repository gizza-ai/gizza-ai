import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

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

test('voice-note-converter page encodes mp3 to Opus voice note', async ({ page }) => {
  await page.goto('/tools/voice-note-converter/');
  await page.waitForSelector('#in-audio');
  await page.selectOption('#in-format', 'opus');
  await page.fill('#in-bitrate', '24');
  await expect(page.locator('#in-mono')).toBeChecked();
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(2.8);
  expect(duration).toBeLessThan(3.3);
});

test('voice-note-converter deep link prefills mp3 bitrate and mono=false', async ({ page }) => {
  await page.goto('/tools/voice-note-converter/?format=mp3&bitrate=160&mono=false');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-format')).toHaveValue('mp3', { timeout: 15_000 });
  await expect(page.locator('#in-bitrate')).toHaveValue('160');
  await expect(page.locator('#in-mono')).not.toBeChecked();
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodeDuration(page, src!);
  expect(duration).toBeGreaterThan(2.8);
  expect(duration).toBeLessThan(3.3);
});

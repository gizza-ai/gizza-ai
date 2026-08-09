import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.wav');

async function decodeAudio(page: Page, src: string): Promise<{ duration: number; bytes: number }> {
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

async function expectAudioOutput(page: Page, mime: RegExp): Promise<{ duration: number; bytes: number }> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(mime);
  return decodeAudio(page, src!);
}

test('audio-declick page runs the default declicker and preserves duration', async ({ page }) => {
  await page.goto('/tools/audio-declick/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', FIXTURE);
  const { duration, bytes } = await expectAudioOutput(page, /^data:audio\/mpeg/);
  expect(bytes).toBeGreaterThan(2_000);
  expect(duration).toBeGreaterThan(2.5);
  expect(duration).toBeLessThan(3.5);
});

test('audio-declick deep link honors aggressive save/declip wav settings', async ({ page }) => {
  await page.goto('/tools/audio-declick/?strength=85&window_ms=25&burst=0&method=save&declip=true&format=wav');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-strength')).toHaveValue('85', { timeout: 15_000 });
  await expect(page.locator('#in-window_ms')).toHaveValue('25');
  await expect(page.locator('#in-burst')).toHaveValue('0');
  await expect(page.locator('#in-method')).toHaveValue('save');
  await expect(page.locator('#in-declip')).toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-file', FIXTURE);
  const { duration, bytes } = await expectAudioOutput(page, /^data:audio\/wav/);
  expect(bytes).toBeGreaterThan(20_000);
  expect(duration).toBeGreaterThan(2.5);
  expect(duration).toBeLessThan(3.5);
});

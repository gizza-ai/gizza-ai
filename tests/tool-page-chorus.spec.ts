import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3');

async function decodedDuration(page: Page, dataUrl: string): Promise<number> {
  return page.evaluate(async (src) => {
    const buf = await (await fetch(src)).arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return decoded.duration;
  }, dataUrl);
}

test('chorus page renders real audio output for an uploaded file', async ({ page }) => {
  await page.goto('/tools/chorus/');
  await page.fill('#in-voices', '2');
  await page.fill('#in-delay_ms', '50');
  await page.fill('#in-depth_ms', '2');
  await page.fill('#in-speed_hz', '0.4');
  await page.fill('#in-decay', '0.4');
  await page.selectOption('#in-format', 'mp3');
  await page.setInputFiles('#in-audio', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodedDuration(page, src!);
  expect(duration).toBeGreaterThan(2.5);
  expect(duration).toBeLessThan(4.0);
});

test('chorus deep-link preloads advanced params and renders wav output', async ({ page }) => {
  await page.goto('/tools/chorus/?voices=4&delay_ms=55&depth_ms=5&speed_hz=0.7&decay=0.55&format=wav');
  await expect(page.locator('#in-voices')).toHaveValue('4', { timeout: 15_000 });
  await expect(page.locator('#in-delay_ms')).toHaveValue('55');
  await expect(page.locator('#in-depth_ms')).toHaveValue('5');
  await expect(page.locator('#in-speed_hz')).toHaveValue('0.7');
  await expect(page.locator('#in-decay')).toHaveValue('0.55');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-audio', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await decodedDuration(page, src!);
  expect(duration).toBeGreaterThan(2.5);
  expect(duration).toBeLessThan(4.0);
});

import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3');

async function durationOf(page: Page, dataUrl: string): Promise<number> {
  return page.evaluate(async (src) => {
    const buf = await (await fetch(src)).arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return decoded.duration;
  }, dataUrl);
}

test('audio-reverse page reverses an uploaded clip to mp3', async ({ page }) => {
  await page.goto('/tools/audio-reverse/');
  await page.waitForSelector('#in-audio');
  await page.selectOption('#in-mode', 'reverse');
  await page.selectOption('#in-format', 'mp3');

  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await durationOf(page, src!);
  expect(duration).toBeGreaterThan(2.8);
  expect(duration).toBeLessThan(3.3);
});

test('audio-reverse deep link doubles duration for reverse-forward wav', async ({ page }) => {
  await page.goto('/tools/audio-reverse/?mode=reverse-forward&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-mode')).toHaveValue('reverse-forward', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('wav');

  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await durationOf(page, src!);
  expect(duration).toBeGreaterThan(5.8);
  expect(duration).toBeLessThan(6.4);
});

test('audio-reverse advertised mode and format controls stay wired', async ({ page }) => {
  await page.goto('/tools/audio-reverse/');
  await page.waitForSelector('#in-audio');
  await page.selectOption('#in-mode', 'forward-reverse');
  await page.selectOption('#in-format', 'ogg');
  await expect(page.locator('#in-mode')).toHaveValue('forward-reverse');
  await expect(page.locator('#in-format')).toHaveValue('ogg');

  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const duration = await durationOf(page, src!);
  expect(duration).toBeGreaterThan(5.8);
  expect(duration).toBeLessThan(6.5);
});

test('audio-reverse generated CLI example is generic and parseable', async ({ page }) => {
  await page.goto('/tools/audio-reverse/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool audio-reverse');
  expect(cli).toContain('url=https://example.com/input');
  expect(cli).toContain('mode=reverse');
  expect(cli).toContain('format=mp3');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
